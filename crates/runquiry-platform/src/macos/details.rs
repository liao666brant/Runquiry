//! macOS 进程详情（`ProcessDetailsProvider`）。
//!
//! 来源（witr `process_darwin.go` / `extended_darwin.go` 语义）：
//! * exe：`proc_pidpath`（witr 用 lsof `txt` 行；同为内核权威路径，属披露的
//!   等价替换，且省去一次 lsof 进程扫描）；
//! * cwd：`lsof -a -p <pid> -d cwd,txt -F fn`（**非零退出但 stdout 有数据时
//!   抢救**，witr 同语义）；
//! * 内存 / 线程：`proc_pidinfo(PROC_PIDTASKINFO)`；I/O：`proc_pid_rusage`；
//!   FD 总数：`PROC_PIDLISTFDS`；FD 路径列表：`lsof -F fn`（前 10 条，witr
//!   `formatFDEntries` 的 10 条上限；libproc `PROC_PIDFDVNODEPATHINFO` 的
//!   `vnode_fdinfowithpath` 布局不手写，属披露的 best-effort 替换）；
//! * FD 软上限：`launchctl limit maxfiles`（witr 的 `sh -c ulimit` 兜底在
//!   Runquiry 的无 shell 边界下省略，失败记诊断）；
//! * 环境变量：`ps -E`（SIP 受限时不可得，值绝不写日志）；
//! * 子进程：sysinfo PPID 扫描；祖先链由 core 管线经快照解析。
//!
//! 硬失败语义：查询期间进程退出 → [`InspectError::NotFound`]；身份重读发现
//! 启动时间不一致 → [`InspectError::ProcessChanged`]，不返回旧数据。

use std::path::PathBuf;

use runquiry_core::{
    CapabilityStatus, CommandSpec, DiagnosticCode, DiagnosticIssue, InspectError, Inspection,
    IoStats, LIST_TIMEOUT, MemoryInfo, PROBE_TIMEOUT, ProcessDetails, ProcessDetailsProvider,
    ProcessIdentity,
};
use sysinfo::System;

use super::MacosPlatform;
use super::identity;
use super::launchctl;
use super::libproc;
use super::lsof;

impl ProcessDetailsProvider for MacosPlatform {
    fn capability(&self) -> CapabilityStatus {
        CapabilityStatus::Partial(String::from(
            "cwd/环境变量/FD 路径受 SIP 与 lsof best-effort 限制；失败字段保留其余数据并记诊断",
        ))
    }

    fn details(
        &self,
        expected: &runquiry_core::ProcessIdentity,
    ) -> Result<Inspection<ProcessDetails>, InspectError> {
        let pid = expected.pid().get();
        let system = self.snapshot();
        let process =
            system
                .process(sysinfo::Pid::from_u32(pid))
                .ok_or_else(|| InspectError::NotFound {
                    subject: format!("进程 {pid}"),
                })?;
        let current_start = identity::start_time_from_unix_seconds(process.start_time());
        // 身份重读：仅比较启动时间（trait 契约；exe 是展示位）。current 为
        // None（秒数 0，不可验证）时不误报变化，也按旧数据不得返回的语义
        // 继续采集——与 Linux details 相同的宽松比对。
        let exe = libproc::pid_path(pid).ok().flatten().map(PathBuf::from);
        let current = runquiry_core::ProcessIdentity::new(expected.pid(), current_start, exe);
        if let (Some(expected_start), Some(current_start)) =
            (expected.start_time(), current.start_time())
            && expected_start != current_start
        {
            return Err(InspectError::ProcessChanged { identity: current });
        }

        let mut partial = PartialDetails::new(pid);
        let task = partial.libproc("taskinfo", libproc::task_snapshot(pid));
        let memory_rss_bytes = task
            .as_ref()
            .map(|snapshot| snapshot.resident_bytes)
            .or_else(|| Some(process.memory()).filter(|memory| *memory > 0));
        let total_memory = System::total_memory();
        let memory_percent = match (memory_rss_bytes, total_memory > 0) {
            #[allow(clippy::cast_precision_loss)]
            (Some(rss), true) => Some(rss as f64 / total_memory as f64 * 100.0),
            _ => None,
        };
        let working_dir = self.cwd_txt(pid, &mut partial);
        let environment = partial
            .external("environment", self.environment_of(pid))
            .unwrap_or_default();
        let fd_count = partial.libproc("fdcount", libproc::fd_count(pid));
        let (open_files, fd_count) = self.open_file_paths(pid, &mut partial, fd_count);
        let fd_limit = partial.external("fdlimit", self.maxfiles_limit()).flatten();
        let io = partial.external("io", self.disk_io(pid)).flatten();
        let children = collect_children(pid, &super::ppid_map(&system));

        Ok(partial.finish(ProcessDetails {
            identity: current,
            cpu_percent: None,
            memory_rss_bytes,
            memory_percent,
            working_dir,
            environment,
            children,
            memory: task.map(|snapshot| MemoryInfo {
                vms_bytes: snapshot.virtual_bytes,
                rss_bytes: snapshot.resident_bytes,
                ..MemoryInfo::default()
            }),
            io,
            open_files,
            fd_count,
            fd_limit,
        }))
    }
}

impl MacosPlatform {
    /// cwd（lsof `cwd,txt -F fn`；Err = lsof 缺失/超时；非零退出有输出时按
    /// witr 抢救语义保留解析结果并记诊断）。
    fn cwd_txt(&self, pid: u32, partial: &mut PartialDetails) -> Option<PathBuf> {
        let output = match self.run_lsof(
            &["-a", "-p", &pid.to_string(), "-d", "cwd,txt", "-F", "fn"],
            LIST_TIMEOUT,
        ) {
            Ok(output) => output,
            Err(error) => {
                partial.record(
                    DiagnosticCode::ExternalToolFailed,
                    format!("进程 {pid} 的 cwd 采集失败：{error}"),
                );
                return None;
            }
        };
        if let Some(issue) = super::exit_salvage_issue(&output, "lsof -F fn") {
            partial.push(issue);
        }
        let (cwd, _txt) = lsof::parse_cwd_txt(&String::from_utf8_lossy(&output.stdout));
        cwd
    }

    /// `ps -E` 环境变量（SIP 受限时输出为空，不记诊断——witr 同语义）。
    fn environment_of(&self, pid: u32) -> Result<Option<Vec<(String, String)>>, InspectError> {
        let output = self.run(
            &CommandSpec::new("ps", ["-p", &pid.to_string(), "-E", "-o", "command="]),
            LIST_TIMEOUT,
        )?;
        Ok(Some(launchctl::parse_env_from_ps_command(
            &String::from_utf8_lossy(&output.stdout),
        )))
    }

    /// 磁盘 I/O（libproc rusage；`Ok(None)` = 无数据）。
    fn disk_io(&self, pid: u32) -> Result<Option<IoStats>, InspectError> {
        libproc::disk_io(pid).map_err(|error| InspectError::Unsupported {
            reason: format!("进程 {pid} 的 I/O 读取失败：{error}"),
        })
    }

    /// `launchctl limit maxfiles` 软上限（0 = unlimited，witr 约定）。
    fn maxfiles_limit(&self) -> Result<Option<u64>, InspectError> {
        let output = self.run(
            &CommandSpec::new("launchctl", ["limit", "maxfiles"]),
            PROBE_TIMEOUT,
        )?;
        Ok(launchctl::parse_limit_maxfiles(&String::from_utf8_lossy(
            &output.stdout,
        )))
    }

    /// FD 路径列表（`lsof -a -p <pid> -F fn`，前 10 条，witr
    /// `formatFDEntries` 的 10 条上限）；libproc 的 fd_count 独立保留。
    fn open_file_paths(
        &self,
        pid: u32,
        partial: &mut PartialDetails,
        fd_count: Option<u64>,
    ) -> (Vec<PathBuf>, Option<u64>) {
        let output = match self.run_lsof(&["-a", "-p", &pid.to_string(), "-F", "fn"], LIST_TIMEOUT)
        {
            Ok(output) => output,
            Err(error) => {
                partial.record(
                    DiagnosticCode::ExternalToolFailed,
                    format!("进程 {pid} 的打开文件列表采集失败：{error}"),
                );
                return (Vec::new(), fd_count);
            }
        };
        if let Some(issue) = super::exit_salvage_issue(&output, "lsof -F fn") {
            partial.push(issue);
        }
        let paths: Vec<PathBuf> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| line.strip_prefix('n'))
            .map(|path| PathBuf::from(path.trim()))
            .take(10)
            .collect();
        (paths, fd_count)
    }
}

/// 子进程 PID 列表（PPID 扫描，按 PID 升序去重）。
fn collect_children(
    pid: u32,
    ppids: &std::collections::HashMap<u32, u32>,
) -> Vec<runquiry_core::Pid> {
    let mut children: Vec<runquiry_core::Pid> = ppids
        .iter()
        .filter(|(_, parent)| **parent == pid)
        .filter_map(|(child, _)| runquiry_core::Pid::new(*child).ok())
        .collect();
    children.sort_unstable();
    children.dedup();
    children
}
/// 字段级部分成功采集器（Linux `PartialDetails` 同构）。
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

    fn push(&mut self, issue: DiagnosticIssue) {
        self.issues.push(issue);
    }

    fn record(&mut self, code: DiagnosticCode, message: String) {
        self.push(DiagnosticIssue::new(code, message));
    }

    /// libproc 调用的部分成功包装：`Ok(None)`（ESRCH/EPERM）记 Unknown 诊断。
    fn libproc<T>(&mut self, field: &str, result: std::io::Result<Option<T>>) -> Option<T> {
        match result {
            Ok(Some(value)) => Some(value),
            Ok(None) => {
                self.push(DiagnosticIssue::new(
                    DiagnosticCode::Unknown,
                    format!(
                        "进程 {} 的 {field} 不可得（进程已退出或权限不足）",
                        self.pid
                    ),
                ));
                None
            }
            Err(error) => {
                self.push(DiagnosticIssue::new(
                    DiagnosticCode::Unknown,
                    format!("进程 {} 的 {field} 读取失败：{error}", self.pid),
                ));
                None
            }
        }
    }

    /// 外部命令字段的部分成功包装：失败记 ExternalToolFailed 诊断。
    fn external<T>(&mut self, field: &str, result: Result<T, InspectError>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                self.push(DiagnosticIssue::new(
                    DiagnosticCode::ExternalToolFailed,
                    format!("进程 {} 的 {field} 采集失败：{error}", self.pid),
                ));
                None
            }
        }
    }

    fn finish(self, details: ProcessDetails) -> Inspection<ProcessDetails> {
        if self.issues.is_empty() {
            Inspection::complete(details)
        } else {
            Inspection::partial(details, self.issues)
        }
    }
}
