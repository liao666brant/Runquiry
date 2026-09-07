//! Windows 来源证据采集（`SourceEvidenceProvider`，SCM）。
//!
//! 平台只采集原始证据，判定在 core（`detect_windows_service` 三级判定）：
//! * 一次 `EnumServicesStatusExW` 全量枚举，经 `scm_parse::dedup_by_pid`
//!   得 PID → 服务条目（svchost 共享宿主按 PID 首写者胜，witr
//!   `serviceMapForPIDs` 同语义）；
//! * 仅对 `ancestry` 中出现的 PID 追加 `QueryServiceConfigW` /
//!   `QueryServiceConfig2W` 查询（查询次数以去重后的祖先链集合为上界）；
//! * 键契约（core 消费，未取得的键省略、绝不伪造）：`service`（→
//!   `Source.name`）、`description`（→ `Source.description`）、
//!   `display_name` / `start_mode` / `binary_path` / `state`（原样透传）；
//! * SCM 不可用或枚举失败 → 返回 default 空证据（不报错：来源识别由 core
//!   按「证据缺失」降级，witr `services_windows.go` 同语义），已披露；
//! * 本模块不采集环境变量，值不进入任何日志。

use std::collections::HashSet;

use runquiry_core::{Pid, ProcessSummary, SourceEvidence, SourceEvidenceProvider};

use super::WindowsPlatform;
use super::ffi_scm;
use super::scm_parse::{self, RawServiceEntry};

impl SourceEvidenceProvider for WindowsPlatform {
    fn evidence(&self, ancestry: &[ProcessSummary]) -> SourceEvidence {
        let mut evidence = SourceEvidence::default();
        // SCM 不可用 / 枚举失败 / 缓冲区损坏 → 空证据：不报错、不伪造
        //（witr 同语义；core 按证据缺失回退基础来源）。
        let Ok(buffer) = ffi_scm::enumerate_services() else {
            return evidence;
        };
        let base = buffer.bytes().as_ptr() as usize;
        let Ok(entries) = scm_parse::parse_enum_buffer(buffer.bytes(), base, buffer.count()) else {
            return evidence;
        };
        // 仅祖先链上的 PID 值得追加 config / description 查询。
        let targets: HashSet<u32> = ancestry
            .iter()
            .map(|process| process.identity.pid().get())
            .collect();
        for (pid_raw, entry) in scm_parse::dedup_by_pid(entries) {
            if !targets.contains(&pid_raw) {
                continue;
            }
            let Ok(pid) = Pid::new(pid_raw) else {
                continue;
            };
            evidence
                .windows_service_by_pid
                .push((pid, service_kv(&entry)));
        }
        evidence
    }
}

/// 单服务的证据键值（config / description 查询失败只省略对应键）。
fn service_kv(entry: &RawServiceEntry) -> Vec<(String, String)> {
    let mut kv = vec![
        (String::from("service"), entry.name.clone()),
        (String::from("display_name"), entry.display_name.clone()),
        (
            String::from("state"),
            String::from(scm_parse::service_state_name(entry.state_raw)),
        ),
    ];
    if let Some(config) = ffi_scm::query_service_config(&entry.name) {
        kv.push((String::from("start_mode"), start_mode_key(config.start_raw)));
        if let Some(binary_path) = config.binary_path {
            kv.push((String::from("binary_path"), binary_path));
        }
    }
    if let Some(description) = ffi_scm::query_service_description(&entry.name) {
        kv.push((String::from("description"), description));
    }
    kv
}

/// `dwStartType` → 证据值（witr `startMode` 风格）：Auto / Demand /
/// Disabled，其余（BOOT / SYSTEM 等驱动启动类型）保留原始数字。
#[must_use]
fn start_mode_key(start_raw: u32) -> String {
    // SERVICE_AUTO_START=2、SERVICE_DEMAND_START=3、SERVICE_DISABLED=4。
    match start_raw {
        2 => String::from("Auto"),
        3 => String::from("Demand"),
        4 => String::from("Disabled"),
        other => format!("{other}"),
    }
}
