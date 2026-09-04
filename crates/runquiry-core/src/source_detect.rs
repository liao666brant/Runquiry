//! 来源识别（parity §5：witr `internal/source/detect.go` 的固定优先级链）。
//!
//! 判定顺序：container → ssh → shell → systemd → launchd → BSD rc →
//! supervisor → cron → Windows service → init，命中即返回；全部未命中返回
//! [`Source::unknown`]（来源识别永不返回空）。core 只做纯判定：launchd、
//! BSD rc 与 Windows service 的平台探测（D-Bus / launchctl / SCM）属平台
//! 职责，core 保留链位置但不判定（恒未命中，与 witr 各平台 stub 行为一致）。

use crate::model::ids::Pid;
use crate::model::process::ProcessSummary;
use crate::model::source::{Source, SourceType};
use crate::port::source::SourceEvidence;
use crate::source_shell::{command_base, detect_shell, env_pairs};

/// 已知 supervisor 命令基名 → 来源标签（parity：`knownSupervisors` 名单逐字对齐）。
const KNOWN_SUPERVISORS: &[(&str, &str)] = &[
    ("pm2", "pm2"),
    ("supervisord", "supervisord"),
    ("supervisor", "supervisord"),
    ("gunicorn", "gunicorn"),
    ("uwsgi", "uwsgi"),
    ("s6-supervise", "s6"),
    ("s6", "s6"),
    ("s6-svscan", "s6"),
    ("runsv", "runit"),
    ("runit", "runit"),
    ("runit-init", "runit"),
    ("openrc", "openrc"),
    ("openrc-init", "openrc"),
    ("monit", "monit"),
    ("circusd", "circus"),
    ("circus", "circus"),
    ("systemd", "systemd service"),
    ("systemctl", "systemd service"),
    ("daemontools", "daemontools"),
    ("initctl", "upstart"),
    ("tini", "tini"),
    ("docker-init", "docker-init"),
    ("podman-init", "podman-init"),
    ("smf", "smf"),
    ("launchd", "launchd"),
    ("god", "god"),
    ("forever", "forever"),
    ("nssm", "nssm"),
];

/// 来源识别单一入口：祖先链（root→target）+ 平台证据 → 来源。
///
/// 证据前置条件：`evidence.cgroup_by_pid` 覆盖祖先链各 PID 的 cgroup 原文
/// （读不到的省略）；`env_by_pid` 覆盖目标及祖先的原始键值；`systemd_running`
/// 为平台判定的 systemd 可用性；`systemd_details` 为 D-Bus 富化键值（core
/// 不解释语义，原样透传进 [`Source::details`]）。
#[must_use]
pub fn detect_source(ancestry: &[ProcessSummary], evidence: &SourceEvidence) -> Source {
    detect_container(ancestry, evidence)
        .or_else(|| detect_ssh(ancestry, evidence))
        .or_else(|| detect_shell(ancestry, evidence))
        .or_else(|| detect_systemd(ancestry, evidence))
        // launchd（macOS）/ BSD rc（FreeBSD）的平台探测属平台职责，core 恒未命中。
        .or_else(|| detect_supervisor(ancestry))
        .or_else(|| detect_cron(ancestry))
        // Windows service（SCM 三级判定）属平台职责，core 恒未命中。
        .or_else(|| detect_init(ancestry))
        .unwrap_or_else(Source::unknown)
}

/// 容器来源：cgroup 逐祖先判定（复用 [`crate::detect_container_from_cgroup`]
/// 的运行时分支语义），Snap / Flatpak 走目标进程环境变量（parity
/// `detectContainer`）。
fn detect_container(ancestry: &[ProcessSummary], evidence: &SourceEvidence) -> Option<Source> {
    for p in ancestry {
        let Some(content) = cgroup_of(p, evidence) else {
            continue;
        };
        // 分支与优先级对齐 witr detectContainer（docker → podman/libpod →
        // kubepods → colima → containerd → lxc.payload，命中即返回）。
        let name = if content.contains("docker") {
            "docker"
        } else if content.contains("podman") || content.contains("libpod") {
            "podman"
        } else if content.contains("kubepods") {
            "kubernetes"
        } else if content.contains("colima") {
            "colima"
        } else if content.contains("containerd") {
            "containerd"
        } else if content.contains("lxc.payload") {
            return Some(
                Source::new(SourceType::Container)
                    .with_name(String::from(lxc_runtime_from_ancestry(ancestry))),
            );
        } else {
            continue;
        };
        return Some(Source::new(SourceType::Container).with_name(String::from(name)));
    }

    // Snap / Flatpak 沙箱：读取目标进程环境变量（parity：SNAP_NAME=/FLATPAK_ID=）。
    let target = ancestry.last()?;
    for (key, _) in env_pairs(target, evidence) {
        if key == "SNAP_NAME" {
            return Some(Source::new(SourceType::Container).with_name(String::from("snap")));
        }
        if key == "FLATPAK_ID" {
            return Some(Source::new(SourceType::Container).with_name(String::from("flatpak")));
        }
    }
    None
}

/// LXC 系运行时精化：祖先命令为 incusd / lxd / lxc-start 时分别判定
/// incus / lxd / lxc，无命中回退 "lxc"（parity `detectLXCRuntime`）。
fn lxc_runtime_from_ancestry(ancestry: &[ProcessSummary]) -> &'static str {
    for p in ancestry {
        match command_base(&p.command) {
            "incusd" => return "incus",
            "lxd" => return "lxd",
            "lxc-start" => return "lxc",
            _ => {}
        }
    }
    "lxc"
}

/// SSH 来源：祖先链（排除目标自身）存在 `sshd` / `sshd.exe` / `sshd:` 前缀
/// 且链长 ≥2；
/// 连接详情从 `SSH_CLIENT` / `SSH_CONNECTION` / `SSH_TTY` 环境变量自目标向祖先
/// 回溯（su/sudo 清洗环境后仍可从祖先取到，parity `detectSSH`）。
fn detect_ssh(ancestry: &[ProcessSummary], evidence: &SourceEvidence) -> Option<Source> {
    if ancestry.len() < 2 {
        return None;
    }
    let has_sshd = ancestry[..ancestry.len() - 1].iter().any(|p| {
        let base = command_base(&p.command);
        base == "sshd" || base == "sshd.exe" || base.starts_with("sshd:")
    });
    if !has_sshd {
        return None;
    }

    let target = ancestry.last()?;
    let user = target.user.as_deref().unwrap_or_default();
    let mut remote_ip = "";
    let mut tty = "";
    for p in ancestry.iter().rev() {
        if !remote_ip.is_empty() {
            break; // parity：找到远端 IP 即停止回溯
        }
        for (key, value) in env_pairs(p, evidence) {
            match key.as_str() {
                // SSH_CLIENT 与 SSH_CONNECTION 的远端 IP 同为首个空白分隔段。
                "SSH_CLIENT" | "SSH_CONNECTION" if remote_ip.is_empty() => {
                    remote_ip = value.split_whitespace().next().unwrap_or("");
                }
                "SSH_TTY" if tty.is_empty() => {
                    tty = value.as_str();
                }
                _ => {}
            }
        }
    }

    // 描述组装对齐 witr（intentional change：英文文案由 UI 呈现）。
    let description = if remote_ip.is_empty() {
        String::from("SSH session")
    } else if !tty.is_empty() {
        format!(
            "SSH session from {remote_ip} ({user}@{})",
            tty.strip_prefix("/dev/").unwrap_or(tty)
        )
    } else if !user.is_empty() {
        format!("SSH session from {remote_ip} ({user})")
    } else {
        format!("SSH session from {remote_ip}")
    };
    Some(
        Source::new(SourceType::Ssh)
            .with_name(String::from("sshd"))
            .with_description(description),
    )
}

/// systemd 来源：systemd 在运行 + 祖先链含 PID 1 + 单元名来自目标 cgroup
/// （parity `detectSystemd`：`IsSystemdRunning` 与 hasPID1 前提）；D-Bus 富化
/// 键值原样透传进 details，core 不解释语义。
fn detect_systemd(ancestry: &[ProcessSummary], evidence: &SourceEvidence) -> Option<Source> {
    if !evidence.systemd_running {
        return None;
    }
    if !ancestry.iter().any(|p| p.identity.pid() == Pid::MIN) {
        return None;
    }
    let target = ancestry.last()?;
    let unit = cgroup_of(target, evidence).and_then(crate::systemd_unit_from_cgroup);
    let mut source = Source::new(SourceType::Systemd);
    if let Some(unit) = unit {
        source = source.with_name(unit);
    }
    for (key, value) in &evidence.systemd_details {
        source = source.with_detail(key.clone(), value.clone());
    }
    Some(source)
}

/// supervisor 来源（parity `detectSupervisor`）：祖先命令基名或 cmdline
/// token 命中已知 supervisor 名单；`init` 仅在链中无 shell 时作为 supervisor
/// 上报（有 shell 视为用户手工启动，继续寻找）。
fn detect_supervisor(ancestry: &[ProcessSummary]) -> Option<Source> {
    let has_shell = ancestry
        .iter()
        .any(|p| crate::source_shell::is_shell(&command_base(&p.command).to_lowercase()));
    for p in ancestry {
        let base = command_base(&p.command);
        if base == "init" && !has_shell {
            return Some(supervisor_source("init"));
        }
        let lower_base = base.to_lowercase();
        if let Some((_, label)) = KNOWN_SUPERVISORS.iter().find(|(key, _)| *key == lower_base) {
            if *label == "init" && has_shell {
                continue;
            }
            return Some(supervisor_source(label));
        }
        if let Some(cmdline) = p.command_line.as_ref()
            && let Some(label) = match_cmdline_tokens(cmdline, has_shell)
        {
            return Some(supervisor_source(label));
        }
    }
    None
}

/// cmdline 逐 token 查 supervisor 名单（跳过 flag 与环境赋值；parity
/// `matchCmdlineTokens`）。
fn match_cmdline_tokens(cmdline: &str, has_shell: bool) -> Option<&'static str> {
    for token in cmdline.to_lowercase().split_whitespace() {
        if token.starts_with('-') || token.contains('=') {
            continue;
        }
        let base = command_base(token);
        if let Some((_, label)) = KNOWN_SUPERVISORS.iter().find(|(key, _)| *key == base) {
            if *label == "init" && has_shell {
                continue;
            }
            return Some(label);
        }
    }
    None
}

/// cron 来源：祖先命令基名 cron / crond（parity `detectCron`）。
fn detect_cron(ancestry: &[ProcessSummary]) -> Option<Source> {
    for p in ancestry {
        let base = command_base(&p.command);
        if base == "cron" || base == "crond" {
            return Some(Source::new(SourceType::Cron).with_name(String::from("cron")));
        }
    }
    None
}

/// init 来源兜底：根进程为 PID 1 且根与目标之间无 shell（有 shell 视为用户
/// 手工运行，不判为 init；witr 的 Windows PID 4 "System" 分支不在 core 判定，
/// 由 Windows 平台证据扩展）。名字取 PID 1 的实际命令名（openrc-init /
/// runit-init / init / systemd 等，parity `detectInit`）。
fn detect_init(ancestry: &[ProcessSummary]) -> Option<Source> {
    let root = ancestry.first()?;
    if root.identity.pid() != Pid::MIN {
        return None;
    }
    let has_shell = ancestry
        .get(1..ancestry.len().saturating_sub(1))
        .is_some_and(|middle| {
            middle
                .iter()
                .any(|p| crate::source_shell::is_shell(&command_base(&p.command).to_lowercase()))
        });
    if has_shell {
        return None;
    }
    // 无 shell 介入时使用 PID 1 的实际命令名（openrc-init / runit-init 等）。
    let init_name = if root.command.is_empty() {
        String::from("init")
    } else {
        root.command.clone()
    };
    Some(
        Source::new(SourceType::Init)
            .with_name(init_name)
            .with_detail(String::from("pid"), root.identity.pid().get().to_string())
            .with_detail(String::from("comm"), root.command.clone()),
    )
}

/// 读取某祖先进程的 cgroup 原文（证据缺失时 `None`）。
fn cgroup_of<'a>(summary: &ProcessSummary, evidence: &'a SourceEvidence) -> Option<&'a str> {
    evidence
        .cgroup_by_pid
        .iter()
        .find(|(pid, _)| *pid == summary.identity.pid())
        .map(|(_, text)| text.as_str())
}

/// supervisor 来源构造（统一 Name 字段）。
fn supervisor_source(label: &str) -> Source {
    Source::new(SourceType::Supervisor).with_name(String::from(label))
}
