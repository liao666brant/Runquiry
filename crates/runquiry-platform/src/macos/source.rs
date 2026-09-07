//! macOS 来源证据采集（`SourceEvidenceProvider`）。
//!
//! 平台只采集原始证据，判定在 core（[`runquiry_core::detect_source`]）：
//! * `cgroup_by_pid` 留空——macOS 无 `/proc`，core 的容器 / systemd 单元
//!   判定按「证据缺失」处理；
//! * `env_by_pid` 仅在 `ps -E` 确实可得时填充（SIP 受限时为空，witr
//!   `getEnvironment` 同语义）；**值绝不写入日志或诊断消息**；
//! * `launchd_by_pid` 按 core 键契约填充（`label` / `comment` / `plist` /
//!   `type` / `schedule` / `triggers` / `keepalive`），判定本身（祖先链
//!   根为 PID 1 launchd）由 core 完成。
//!
//! launchd 链路（witr `internal/launchd/plist.go::GetLaunchdInfo`）：
//! `launchctl blame <pid>` → 域/label（blame 原因行返回 None，core 回退
//! 基础 launchd 来源）→ plist 搜索路径定位 → `plutil -convert xml1` →
//! 纯 XML 提取（[`super::plist`]）。plist 缺失或解析失败时保留已取得的
//! label / 域描述（witr 同语义）。

use runquiry_core::{CommandSpec, Pid, ProcessSummary, SourceEvidence, SourceEvidenceProvider};

use super::MacosPlatform;
use super::launchctl;
use super::plist::{self, LaunchdPlistInfo};

impl SourceEvidenceProvider for MacosPlatform {
    fn evidence(&self, ancestry: &[ProcessSummary]) -> SourceEvidence {
        let mut evidence = SourceEvidence::default();
        for process in ancestry {
            if let Some(environment) = self.ps_env_pairs(process.identity.pid()) {
                if !environment.is_empty() {
                    evidence
                        .env_by_pid
                        .push((process.identity.pid(), environment));
                }
            }
        }
        if let Some(target) = ancestry.last()
            && let Some(kv) = self.launchd_evidence(target.identity.pid())
        {
            evidence.launchd_by_pid.push((target.identity.pid(), kv));
        }
        evidence
    }
}

impl MacosPlatform {
    /// 单 PID 的 `ps -E` 环境键值（值不写日志；SIP 受限时为 None/空）。
    fn ps_env_pairs(&self, pid: Pid) -> Option<Vec<(String, String)>> {
        let output = self
            .run(
                &CommandSpec::new("ps", ["-p", &pid.get().to_string(), "-E", "-o", "command="]),
                runquiry_core::LIST_TIMEOUT,
            )
            .ok()?;
        Some(launchctl::parse_env_from_ps_command(
            &String::from_utf8_lossy(&output.stdout),
        ))
    }

    /// 目标 PID 的 launchd 证据键值；非 launchd 托管进程返回 `None`
    /// （core 回退基础来源或不命中 launchd 来源）。
    fn launchd_evidence(&self, pid: Pid) -> Option<Vec<(String, String)>> {
        let blame = self
            .run(
                &CommandSpec::new("launchctl", ["blame", &pid.get().to_string()]),
                runquiry_core::PROBE_TIMEOUT,
            )
            .ok()?;
        let stdout = String::from_utf8_lossy(&blame.stdout);
        let (domain, label) =
            launchctl::parse_blame_service(&stdout).or_else(|| self.launchd_list_fallback(pid))?;
        Some(self.launchd_kv(&domain, &label))
    }

    /// `launchctl list` 回退（witr `findServiceByPID`）：blame 输出为原因
    /// 行时按 PID 找 label；域按 witr 约定（`com.apple.` 前缀归 system）。
    fn launchd_list_fallback(&self, pid: Pid) -> Option<(String, String)> {
        let output = self
            .run(
                &CommandSpec::new("launchctl", ["list"]),
                runquiry_core::LIST_TIMEOUT,
            )
            .ok()?;
        let label =
            launchctl::parse_list_label(&String::from_utf8_lossy(&output.stdout), pid.get())?;
        let domain = if label.starts_with("com.apple.") {
            String::from("system")
        } else {
            String::from("user")
        };
        Some((domain, label))
    }

    /// 组装 launchd 证据键值（label / plist / plist 富化 / 触发器）。
    fn launchd_kv(&self, domain: &str, blamed_label: &str) -> Vec<(String, String)> {
        let mut kv: Vec<(String, String)> =
            vec![(String::from("type"), launchctl::domain_description(domain))];
        let mut plist_found = None;
        let mut info = LaunchdPlistInfo::default();
        for candidate in launchctl::plist_candidates(&blamed_label) {
            if std::fs::metadata(&candidate).is_ok() {
                plist_found = Some(candidate);
                break;
            }
        }
        if let Some(path) = plist_found.as_ref() {
            if let Some(converted) = self.read_plist_xml(path) {
                info = plist::parse_plist_xml(&converted);
            }
        }
        // witr：来源名取 plist 的 Label；plist 缺失/未提供 Label 时回退
        // blame 解析出的 label。
        let label = if info.label.is_empty() {
            blamed_label
        } else {
            info.label
        };
        kv.push((String::from("label"), label));
        if let Some(path) = plist_found {
            kv.push((String::from("plist"), path));
        }
        if !info.comment.is_empty() {
            kv.push((String::from("comment"), info.comment));
        }
        let triggers = plist::format_triggers(&info);
        let (schedule, triggers): (Vec<_>, Vec<_>) = triggers.into_iter().partition(|trigger| {
            trigger.starts_with("StartInterval") || trigger.starts_with("StartCalendarInterval")
        });
        if !schedule.is_empty() {
            kv.push((String::from("schedule"), schedule.join("; ")));
        }
        if !triggers.is_empty() {
            kv.push((String::from("triggers"), triggers.join("; ")));
        }
        if info.keep_alive {
            kv.push((
                String::from("keepalive"),
                String::from("Yes (restarts if killed)"),
            ));
        }
        kv
    }

    /// `plutil -convert xml1 -o - <path>`（二进制 plist 统一为 XML；失败为
    /// None，调用方按 witr「plist 解析失败保留基础信息」处理）。
    fn read_plist_xml(&self, path: &str) -> Option<String> {
        let output = self
            .run(
                &CommandSpec::new("plutil", ["-convert", "xml1", "-o", "-", path]),
                runquiry_core::LIST_TIMEOUT,
            )
            .ok()?;
        Some(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// launchd 服务名 → 运行中 PID（witr `resolveLaunchdServicePID`）：
    /// 先校验 label（防注入），再依次尝试 name / com.apple.name / org.name /
    /// io.name 四候选的 `launchctl print system/<label>`，从输出解析
    /// `pid = <n>`。
    ///
    /// 接入点：名称解析（core `resolve_name` 的 `service_pid` 参数）零命中
    /// 后的服务回退由 app 装配层调用（`merge_service_pid` 排首位）；app
    /// 接线不在 C1 写入范围，由主代理后续执行。
    ///
    /// # Errors
    /// label 非法 → [`InspectError::InvalidTarget`]；launchctl 缺失/超时 →
    /// [`InspectError::ExternalTool`]；四个候选均无运行实例 → `Ok(None)`。
    pub fn launchd_service_pid(&self, name: &str) -> Result<Option<Pid>, InspectError> {
        if !launchctl::is_valid_service_label(name) {
            return Err(InspectError::InvalidTarget {
                reason: format!(
                    "launchd label {name:?} 非法（仅允许字母数字与 . _ -，长度 1..=256）"
                ),
            });
        }
        for label in launchctl::candidate_labels(name) {
            let output = self.run(
                &CommandSpec::new("launchctl", ["print", &format!("system/{label}")]),
                runquiry_core::PROBE_TIMEOUT,
            )?;
            // witr：仅服务存在（退出码 0）的 print 输出参与解析。
            if output.exit_code == Some(0)
                && let Some(pid) =
                    launchctl::parse_print_pid(&String::from_utf8_lossy(&output.stdout))
                && let Ok(pid) = Pid::new(pid)
            {
                return Ok(Some(pid));
            }
        }
        Ok(None)
    }
}
