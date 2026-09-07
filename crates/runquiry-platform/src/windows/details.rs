//! Windows 进程详情：`ProcessDetailsProvider` 的 PEB 安全包装 + PSAPI 实现。
//!
//! 硬身份失败（权限 / 进程消失 / PID 复用）与字段级部分成功：可打开的进程
//! 逐字段采集（失败只加诊断）；不可打开的进程以 sysinfo 基线字段返回部分
//! 结果并附 `PermissionDenied` / 受保护进程诊断（witr GetProcessDetailedInfo
//! 的分级回退语义）。

use std::time::SystemTime;

use runquiry_core::{
    CapabilityStatus, DiagnosticCode, DiagnosticIssue, InspectError, Inspection, MemoryInfo, Pid,
    ProcessDetails, ProcessDetailsProvider, ProcessIdentity,
};
use sysinfo::{ProcessesToUpdate, System};

use super::WindowsPlatform;
use super::ffi::{self, HandleGuard};
use super::peb_reader;
use super::process_list::sysinfo_snapshot;

/// 逐字段部分成功采集器。
struct PartialDetails {
    pid: u32,
    issues: Vec<DiagnosticIssue>,
}

impl PartialDetails {
    const fn new(pid: u32) -> Self {
        Self {
            pid,
            issues: Vec::new(),
        }
    }

    fn record(&mut self, code: DiagnosticCode, field: &str, reason: String) {
        self.issues.push(DiagnosticIssue::new(
            code,
            format!("进程 {} 的 {field}：{reason}", self.pid),
        ));
    }

    fn finish(self, details: ProcessDetails) -> Inspection<ProcessDetails> {
        if self.issues.is_empty() {
            Inspection::complete(details)
        } else {
            Inspection::partial(details, self.issues)
        }
    }
}

/// 仅刷新单个 PID 的 sysinfo 快照（目标进程必须存在）。
fn fresh_process(pid: u32) -> Option<System> {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::Some(&[sysinfo::Pid::from_u32(pid)]));
    let found = system.process(sysinfo::Pid::from_u32(pid)).is_some();
    found.then_some(system)
}

/// 从 sysinfo 取当前身份的启动时间与可执行路径（PID 复用比较与回退共用）。
fn current_identity(pid: u32) -> Option<(Option<SystemTime>, Option<std::path::PathBuf>)> {
    let system = fresh_process(pid)?;
    let process = system.process(sysinfo::Pid::from_u32(pid))?;
    let start = super::process_list::start_time_from_sysinfo(process.start_time());
    Some((start, process.exe().map(std::path::Path::to_path_buf)))
}

impl ProcessDetailsProvider for WindowsPlatform {
    fn capability(&self) -> CapabilityStatus {
        CapabilityStatus::Supported
    }

    fn details(
        &self,
        identity: &ProcessIdentity,
    ) -> Result<Inspection<ProcessDetails>, InspectError> {
        let pid = identity.pid().get();
        let subject = format!("进程 {pid}");
        let Some((current_start, sysinfo_exe)) = current_identity(pid) else {
            return Err(InspectError::NotFound { subject });
        };
        // PID 复用防护：仅当两侧启动时间均可得且不一致时判定身份变化
        // （同 core 的 same_process 保守语义）。
        if let (Some(expected), Some(current)) = (identity.start_time(), current_start)
            && expected != current
        {
            return Err(InspectError::ProcessChanged {
                identity: ProcessIdentity::new(identity.pid(), Some(current), sysinfo_exe),
            });
        }
        let mut partial = PartialDetails::new(pid);
        match ffi::open_process_graded(pid) {
            Ok(handle) => {
                let details = self.collect_with_handle(
                    pid,
                    &handle,
                    current_start,
                    sysinfo_exe,
                    &mut partial,
                );
                Ok(partial.finish(details))
            }
            Err(error) => {
                // 受保护进程 / 权限不足：以 sysinfo 基线返回部分结果。
                partial.record(
                    if error.is_access_denied() {
                        DiagnosticCode::PermissionDenied
                    } else {
                        DiagnosticCode::Unknown
                    },
                    "句柄",
                    format!(
                        "无法打开进程（Win32 错误码 {}；{}）",
                        error.0,
                        if error.is_access_denied() {
                            "受保护进程或权限不足"
                        } else {
                            "进程状态受限"
                        }
                    ),
                );
                let details = self.collect_sysinfo_only(pid, current_start, sysinfo_exe);
                Ok(partial.finish(details))
            }
        }
    }
}

impl WindowsPlatform {
    /// 句柄可得时的完整采集（逐字段失败保留其余数据）。
    fn collect_with_handle(
        &self,
        pid: u32,
        handle: &HandleGuard,
        current_start: Option<SystemTime>,
        sysinfo_exe: Option<std::path::PathBuf>,
        partial: &mut PartialDetails,
    ) -> ProcessDetails {
        // exe：QueryFullProcessImageNameW 优先，失败回退 sysinfo 路径
        //（witr exePath / snapExe 分级；回退本身记权限类诊断）。
        let exe_path = match ffi::query_full_image_name(handle) {
            Some(path) => Some(std::path::PathBuf::from(path)),
            None => {
                partial.record(
                    DiagnosticCode::PermissionDenied,
                    "可执行路径",
                    String::from("QueryFullProcessImageNameW 失败，回退 sysinfo"),
                );
                sysinfo_exe
            }
        };
        // PEB：命令行 / 工作目录 / 环境块（有界；读取中退出 → 部分结果，
        // 单字段失败只加诊断，不伪造数据）。
        let peb = peb_reader::read_remote_strings(handle);
        partial.issues.extend(peb.issues);
        // 内存（PSAPI）：RSS = WorkingSetSize，VMS = PrivateUsage（witr
        // extended_windows.go 语义）；其余字段 Windows 无对应来源，记 0。
        let memory = ffi::memory_counters(handle).map(|(working_set, private_usage)| MemoryInfo {
            vms_bytes: private_usage,
            rss_bytes: working_set,
            shared_bytes: 0,
            text_bytes: 0,
            lib_bytes: 0,
            data_bytes: 0,
            dirty_bytes: 0,
        });
        if memory.is_none() {
            partial.record(
                DiagnosticCode::Unknown,
                "内存计数",
                String::from("GetProcessMemoryInfo 失败"),
            );
        }
        let memory_rss_bytes = memory.map(|info| info.rss_bytes);
        // CPU：witr GetResourceContext 的生命周期平均（CPU 时间 / 存活时长）。
        let cpu_percent = ffi::get_process_times(handle).and_then(|(started, cpu_time)| {
            let started = started?;
            let wall = started.elapsed().ok()?;
            (wall.as_nanos() > 0).then(|| {
                #[allow(clippy::cast_precision_loss)]
                {
                    cpu_time.as_secs_f64() / wall.as_secs_f64() * 100.0
                }
            })
        });
        let io = ffi::io_counters(handle);
        if io.is_none() {
            partial.record(
                DiagnosticCode::Unknown,
                "I/O 计数",
                String::from("GetProcessIoCounters 失败"),
            );
        }
        // Windows 无 FD 枚举（parity §10 Unsupported，open_files 恒空、
        // fd_limit 恒未取得）；fd_count 以句柄数采集（witr
        // extended_windows.go 的 FDCount = handle count 语义）。
        let fd_count = ffi::handle_count(handle).map(u64::from);
        ProcessDetails {
            identity: ProcessIdentity::new(
                Pid::new(pid).unwrap_or(Pid::MIN),
                current_start,
                exe_path,
            ),
            cpu_percent,
            memory_rss_bytes,
            memory_percent: memory_percent_of(memory_rss_bytes),
            working_dir: peb.working_dir.map(std::path::PathBuf::from),
            environment: peb.environment.unwrap_or_default(),
            children: self.collect_children(pid),
            memory,
            io,
            open_files: Vec::new(),
            fd_count,
            fd_limit: None,
        }
    }

    /// 句柄不可得时的 sysinfo 基线部分结果（不含 PEB / PSAPI 字段）。
    fn collect_sysinfo_only(
        &self,
        pid: u32,
        current_start: Option<SystemTime>,
        sysinfo_exe: Option<std::path::PathBuf>,
    ) -> ProcessDetails {
        let system = fresh_process(pid);
        let rss = system
            .as_ref()
            .and_then(|system| system.process(sysinfo::Pid::from_u32(pid)))
            .map_or(0, |process| process.memory());
        let memory = (rss > 0).then_some(MemoryInfo {
            vms_bytes: 0,
            rss_bytes: rss,
            shared_bytes: 0,
            text_bytes: 0,
            lib_bytes: 0,
            data_bytes: 0,
            dirty_bytes: 0,
        });
        let memory_rss_bytes = (rss > 0).then_some(rss);
        ProcessDetails {
            identity: ProcessIdentity::new(
                Pid::new(pid).unwrap_or(Pid::MIN),
                current_start,
                sysinfo_exe,
            ),
            cpu_percent: None,
            memory_rss_bytes,
            memory_percent: memory_percent_of(memory_rss_bytes),
            working_dir: None,
            environment: Vec::new(),
            children: self.collect_children(pid),
            memory,
            io: None,
            open_files: Vec::new(),
            fd_count: None,
            fd_limit: None,
        }
    }

    /// 子进程（sysinfo 快照中 parent == pid 的条目，按 PID 升序）。
    fn collect_children(&self, pid: u32) -> Vec<Pid> {
        let system = sysinfo_snapshot();
        let mut children: Vec<Pid> = system
            .processes()
            .iter()
            .filter_map(|(child, process)| {
                (process.parent()?.as_u32() == pid).then_some(child.as_u32())
            })
            .filter_map(|raw| Pid::new(raw).ok())
            .collect();
        children.sort_unstable();
        children
    }
}

/// 内存占物理内存百分比（witr windowsMemoryPercent：GlobalMemoryStatusEx
/// 总量，进程生命周期内恒定故缓存一次）；总量不可得为 `None`。
fn memory_percent_of(rss: Option<u64>) -> Option<f64> {
    let total = ffi::total_physical_memory();
    let rss = rss?;
    if total == 0 {
        return None;
    }
    #[allow(clippy::cast_precision_loss)]
    Some(rss as f64 / total as f64 * 100.0)
}
