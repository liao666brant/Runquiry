//! Windows 进程基线：`ProcessInventory` 的 sysinfo + ToolHelp32 实现。

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use runquiry_core::{
    CapabilityStatus, DiagnosticCode, DiagnosticIssue, HealthStatus, Inspection, Pid,
    ProcessIdentity, ProcessInventory, ProcessSummary,
};
use sysinfo::{ProcessesToUpdate, System, Users};

use super::WindowsPlatform;
use super::ffi;

/// sysinfo 一次全量刷新（生产枚举基线；witr 快照缓存的等价物由调用方的
/// 单次 `list()` 边界承担，不引入 TTL 缓存）。
pub(super) fn sysinfo_snapshot() -> System {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All);
    system
}

/// 生产 PID 枚举（构造时基准快照与 `list()` 共用）。
pub(super) fn sysinfo_pids() -> Vec<u32> {
    let mut pids: Vec<u32> = sysinfo_snapshot()
        .processes()
        .keys()
        .map(|pid| pid.as_u32())
        .collect();
    pids.sort_unstable();
    pids
}

/// sysinfo 启动秒数 → 身份启动时间（0 = 不可得 → `None`，与 core
/// `ProcessIdentity` 的保守语义及 macOS 侧 `start_time_from_unix_seconds`
/// 一致：`None` 使 `same_process` 恒拒绝，防止不可验证身份误判未复用）。
pub(super) fn start_time_from_sysinfo(seconds: u64) -> Option<SystemTime> {
    (seconds > 0).then(|| SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
}

/// uid → 用户名（sysinfo 用户表；未收录的 uid 回退数字串，witr `readUser`
/// 的兜底语义）。
fn user_of(uid: Option<&sysinfo::Uid>, users: &Users) -> Option<String> {
    let uid = uid?;
    users.get_user_by_id(uid).map_or_else(
        || Some(uid.to_string()),
        |user| Some(String::from(user.name())),
    )
}

/// 健康标签（witr `windowsHealth` 阈值：累计 CPU > 2h → high-cpu，
/// RSS > 1 GiB → high-mem；Windows 无 zombie/stopped 等价物）。
const fn health_of(rss_bytes: u64, cpu_time: Option<Duration>) -> HealthStatus {
    // Duration 的 PartialOrd 非 const，const fn 内改用 nanos 数值比较
    //（2 小时 = 7.2e12 纳秒，语义与 Duration 比较一致）。
    if let Some(cpu) = cpu_time
        && cpu.as_nanos() > 2 * 60 * 60 * 1_000_000_000u128
    {
        return HealthStatus::HighCpu;
    }
    if rss_bytes > 1024 * 1024 * 1024 {
        return HealthStatus::HighMem;
    }
    HealthStatus::Healthy
}

impl WindowsPlatform {
    /// 当前 PPID 映射（自身后代排除与详情子进程发现共用）。
    pub(super) fn ppid_map(&self) -> HashMap<u32, u32> {
        let system = sysinfo_snapshot();
        system
            .processes()
            .iter()
            .filter_map(|(pid, process)| {
                let parent = process.parent()?;
                Some((pid.as_u32(), parent.as_u32()))
            })
            .collect()
    }

    /// 单个 PID 的累计 CPU 时间（健康标签输入；打不开的进程为 `None`，
    /// 保留条目并按 RSS 判定健康）。
    pub(super) fn cpu_time_of(pid: u32) -> Option<Duration> {
        let handle = ffi::open_process_graded(pid).ok()?;
        ffi::get_process_times(&handle).map(|(_, cpu)| cpu)
    }
}

impl ProcessInventory for WindowsPlatform {
    fn capability(&self) -> CapabilityStatus {
        CapabilityStatus::Supported
    }

    fn list(&self) -> Inspection<Vec<ProcessSummary>> {
        let snapshot = ffi::toolhelp_snapshot();
        let fallback_names = |pid: u32| -> Option<(u32, String)> {
            snapshot
                .as_ref()
                .ok()?
                .iter()
                .find(|entry| entry.pid == pid)
                .map(|entry| (entry.parent_pid, entry.exe_name.clone()))
        };
        let system = sysinfo_snapshot();
        let users = Users::new_with_refreshed_list();
        let parents = self.ppid_map();
        let mut summaries = Vec::new();
        let mut issues = Vec::new();
        for (pid, process) in system.processes().iter() {
            let pid_raw = pid.as_u32();
            let start_secs = process.start_time();
            let start_time = start_time_from_sysinfo(start_secs);
            let parent_raw = process.parent().map_or_else(
                || fallback_names(pid_raw).map(|(parent, _)| parent),
                |parent| Some(parent.as_u32()),
            );
            if self.excluded(pid_raw, start_time, &parents) {
                continue;
            }
            let command = process.name().to_string_lossy().to_string();
            let command_line = join_cmdline(process.cmd());
            let exe = process.exe().map(std::path::Path::to_path_buf);
            let rss = process.memory();
            let summary = ProcessSummary {
                identity: ProcessIdentity::new(
                    Pid::new(pid_raw).unwrap_or(Pid::MIN),
                    start_time,
                    exe,
                ),
                parent_pid: parent_raw.and_then(|raw| Pid::new(raw).ok()),
                command,
                command_line,
                user: user_of(process.user_id(), &users),
                health: health_of(rss, Self::cpu_time_of(pid_raw)),
                container: None,
                exe_deleted: false,
                capabilities: Vec::new(),
            };
            summaries.push(summary);
        }
        // sysinfo 全量刷新失败（不可能）之外的兜底：快照不可用时记诊断，
        // 不以空集合冒充成功。
        if summaries.is_empty() {
            match ffi::toolhelp_snapshot() {
                Ok(entries) => {
                    for entry in entries {
                        if entry.pid == self.own_pid.get() {
                            continue;
                        }
                        let Some(pid) = Pid::new(entry.pid).ok() else {
                            continue;
                        };
                        summaries.push(ProcessSummary {
                            identity: ProcessIdentity::new(pid, None, None),
                            parent_pid: Pid::new(entry.parent_pid).ok(),
                            command: entry.exe_name,
                            command_line: None,
                            user: None,
                            health: HealthStatus::Unknown,
                            container: None,
                            exe_deleted: false,
                            capabilities: Vec::new(),
                        });
                    }
                    issues.push(DiagnosticIssue::new(
                        DiagnosticCode::Unknown,
                        String::from(
                            "sysinfo 未枚举到进程，基线退化为 ToolHelp32 快照（无启动时间）",
                        ),
                    ));
                }
                Err(error) => {
                    issues.push(super::winerror::diagnostic_for(error, "进程基线"));
                }
            }
        }
        if issues.is_empty() {
            Inspection::complete(summaries)
        } else {
            Inspection::partial(summaries, issues)
        }
    }
}

/// sysinfo 的 `cmd()` 切片连接为单行；空命令行为 `None`。
fn join_cmdline(cmd: &[std::ffi::OsString]) -> Option<String> {
    if cmd.is_empty() {
        return None;
    }
    Some(
        cmd.iter()
            .map(|part| part.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// ToolHelp32 快照的真实采集验证（仅 Windows 目标；`#[path]` 纯解析套件
/// 无法覆盖系统调用路径）。只读断言：真实系统快照非空、条目绝大多数带
/// 可执行名（UTF-16 定长数组解码路径在真实数据上抽查）。
#[cfg(all(test, target_os = "windows"))]
mod snapshot_live {
    use super::ffi;

    #[test]
    fn toolhelp_snapshot_enumerates_real_processes() -> Result<(), Box<dyn std::error::Error>> {
        let entries = match ffi::toolhelp_snapshot() {
            Ok(entries) => entries,
            Err(error) => {
                return Err(format!("ToolHelp32 快照失败：Win32 错误码 {}", error.0).into());
            }
        };
        assert!(
            entries.len() > 10,
            "真实系统进程数不应少于 10，实际 {}",
            entries.len()
        );
        let named = entries
            .iter()
            .filter(|entry| !entry.exe_name.is_empty())
            .count();
        assert!(
            named * 2 > entries.len(),
            "绝大多数快照条目应有可执行名（{named}/{}）",
            entries.len()
        );
        Ok(())
    }
}
