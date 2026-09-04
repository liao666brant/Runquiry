//! 单个 Linux 进程摘要的字段级部分成功采集。

use std::time::SystemTime;

use runquiry_core::{DiagnosticIssue, HealthStatus, Pid, ProcessIdentity, ProcessSummary};
use sysinfo::Users;

use super::capabilities::decode_capabilities;
use super::process::{LinuxPlatform, diagnostic_for_field};
use super::procfs::{
    CLK_TCK, PAGE_SIZE, StatInfo, StatusInfo, parse_null_list, parse_status, start_time_from_ticks,
};

pub(super) struct SummaryContext<'a> {
    platform: &'a LinuxPlatform,
    boot: Option<SystemTime>,
    users: &'a Users,
}

impl<'a> SummaryContext<'a> {
    pub(super) const fn new(
        platform: &'a LinuxPlatform,
        boot: Option<SystemTime>,
        users: &'a Users,
    ) -> Self {
        Self {
            platform,
            boot,
            users,
        }
    }

    pub(super) fn build(
        &self,
        pid: u32,
        stat: &StatInfo,
    ) -> (ProcessSummary, Vec<DiagnosticIssue>) {
        let mut issues = Vec::new();
        let status = match self.platform.procfs.read_string(&format!("{pid}/status")) {
            Ok(raw) => parse_status(&raw),
            Err(error) => {
                issues.push(diagnostic_for_field(pid, "status", &error));
                StatusInfo::default()
            }
        };
        let command_line = match self.platform.procfs.read(&format!("{pid}/cmdline")) {
            Ok(bytes) => {
                let line = parse_null_list(&bytes).join(" ");
                (!line.is_empty()).then_some(line)
            }
            Err(error) => {
                issues.push(diagnostic_for_field(pid, "cmdline", &error));
                None
            }
        };
        let (exe, exe_deleted) = match self.platform.procfs.read_link(&format!("{pid}/exe")) {
            Ok(target) => {
                let deleted = target.to_string_lossy().ends_with(" (deleted)");
                (Some(target), deleted)
            }
            Err(error) => {
                issues.push(diagnostic_for_field(pid, "exe", &error));
                (None, false)
            }
        };
        let container = match self.platform.procfs.read_string(&format!("{pid}/cgroup")) {
            Ok(raw) => runquiry_core::detect_container_from_cgroup(&raw),
            Err(error) => {
                issues.push(diagnostic_for_field(pid, "cgroup", &error));
                None
            }
        };
        let start_time = self
            .boot
            .and_then(|boot| start_time_from_ticks(boot, stat.start_ticks));
        let health = health_of(stat);
        let summary = ProcessSummary {
            identity: ProcessIdentity::new(Pid::new(pid).unwrap_or(Pid::MIN), start_time, exe),
            parent_pid: Pid::new(stat.ppid).ok(),
            command: command_of(stat, command_line.as_deref()),
            command_line,
            user: LinuxPlatform::user_of(status.uid, self.users),
            health,
            container,
            exe_deleted,
            capabilities: status
                .cap_eff_hex
                .as_deref()
                .map(decode_capabilities)
                .unwrap_or_default(),
        };
        (summary, issues)
    }
}

fn command_of(stat: &StatInfo, command_line: Option<&str>) -> String {
    if stat.comm.is_empty() {
        command_line
            .and_then(|line| line.split_whitespace().next())
            .and_then(|first| first.rsplit('/').next())
            .unwrap_or_default()
            .to_string()
    } else {
        stat.comm.clone()
    }
}

const fn health_of(stat: &StatInfo) -> HealthStatus {
    match stat.state {
        'Z' => HealthStatus::Zombie,
        'T' => HealthStatus::Stopped,
        _ if (stat.utime + stat.stime) / CLK_TCK > 2 * 60 * 60 => HealthStatus::HighCpu,
        _ if stat.rss_pages.saturating_mul(PAGE_SIZE) > 1024 * 1024 * 1024 => HealthStatus::HighMem,
        _ => HealthStatus::Healthy,
    }
}
