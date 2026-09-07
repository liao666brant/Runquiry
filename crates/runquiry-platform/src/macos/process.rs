//! macOS 进程基线与平台构造（`ProcessInventory` 的 sysinfo 实现）。
//!
//! 进程枚举与字段来源（witr `process_list_darwin.go` 用 `ps -axo` 列格式，
//! Runquiry 以 sysinfo 0.31.4 为基线——总计划既定决策；`start_time` 为平台
//! 秒级粒度，已披露）。自身排除策略对齐 `LinuxPlatform`：构造时快照基准，
//! 排除自身 PID 与构造后派生的短命辅助进程。

use std::io;
use std::time::SystemTime;

use runquiry_core::{
    CapabilityStatus, DiagnosticCode, DiagnosticIssue, Inspection, Pid, ProcessIdentity,
    ProcessInventory, ProcessSummary,
};
use sysinfo::{System, Users};

use crate::command::StdCommandRunner;
use super::{identity, ppid_map, sysinfo_pids};

/// macOS 只读采集适配器：单一结构实现七个只读端口与
/// [`runquiry_core::ProcessController`]。
#[derive(Debug, Clone)]
pub struct MacosPlatform {
    /// Runquiry 自身 PID（恒被排除）。
    pub(super) own_pid: Pid,
    /// 构造时刻的 PID 基准快照。
    pub(super) baseline_pids: Vec<u32>,
    /// 构造时刻（生产为墙钟）。`None` 表示注入式实例（禁用控制副作用）。
    pub(super) constructed_at: Option<SystemTime>,
    /// 受限外部命令执行器（lsof / launchctl / plutil / ps 的唯一通道）。
    pub(super) runner: StdCommandRunner,
}

impl MacosPlatform {
    /// 以生产配置构造：sysinfo 基准快照、自身 PID 与墙钟构造时刻。
    ///
    /// # Errors
    /// 自身 PID 为 0 时返回错误（正常进程不会发生）。
    pub fn new() -> io::Result<Self> {
        let own_pid = Pid::new(std::process::id())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "自身 PID 为 0"))?;
        Ok(Self {
            own_pid,
            baseline_pids: sysinfo_pids(),
            constructed_at: Some(SystemTime::now()),
            runner: StdCommandRunner,
        })
    }

    /// 以显式注入的构造时刻与自身 PID 构造（QA 使用）。
    ///
    /// `constructed_at` 为 `None` 时等价「注入式实例」：进程控制副作用被
    /// 禁用（与 `LinuxPlatform::with_injected` 的只读边界一致）。
    #[must_use]
    pub fn with_injected(constructed_at: Option<SystemTime>, own_pid: Pid) -> Self {
        Self {
            own_pid,
            baseline_pids: sysinfo_pids(),
            constructed_at,
            runner: StdCommandRunner,
        }
    }

    /// uid → 用户名（sysinfo 用户表；表外 uid 回退数字串，witr `resolveUID`
    /// 同语义）。
    pub(super) fn user_of(uid: u32, users: &Users) -> Option<String> {
        sysinfo::Uid::try_from(usize::from(uid))
            .ok()
            .and_then(|uid| users.get_user_by_id(&uid))
            .map_or_else(
                || Some(uid.to_string()),
                |user| Some(String::from(user.name())),
            )
    }
}

impl ProcessInventory for MacosPlatform {
    fn capability(&self) -> CapabilityStatus {
        // sysinfo 原生支持 macOS 进程枚举；lsof/launchctl 依赖在各端口的
        // 采集失败路径如实呈现（不在此预判环境）。
        CapabilityStatus::Supported
    }

    fn list(&self) -> Inspection<Vec<ProcessSummary>> {
        let system = self.snapshot();
        let users = Users::new_with_refreshed_list();
        let ppids = ppid_map(&system);
        let mut summaries = Vec::new();
        let mut issues = Vec::new();
        for process in system.processes().values() {
            let pid = process.pid().as_u32();
            let start_seconds = process.start_time();
            if self.excluded(pid, &ppids, Some(start_seconds)) {
                continue;
            }
            let command = command_of(process);
            if command.is_empty() {
                issues.push(DiagnosticIssue::new(
                    DiagnosticCode::ParseFailed,
                    format!("进程 {pid} 的命令名与命令行均不可得，条目跳过"),
                ));
                continue;
            }
            // exe_deleted（witr `os.Stat(binPath)` 非存在即置位；来源为
            // sysinfo exe——witr 的 lsof txt 来源在基线全量扫描下不可承受）。
            let exe = process.exe().map(std::path::Path::to_path_buf);
            let exe_deleted = exe.as_ref().is_some_and(|path| !path.exists());
            summaries.push(ProcessSummary {
                identity: ProcessIdentity::new(
                    Pid::new(pid).unwrap_or(Pid::MIN),
                    identity::start_time_from_unix_seconds(start_seconds),
                    exe,
                ),
                parent_pid: process
                    .parent()
                    .and_then(|parent| Pid::new(parent.as_u32()).ok()),
                command,
                command_line: cmdline_of(process),
                user: process
                    .user_id()
                    .and_then(|uid| usize::try_from(**uid).ok())
                    .and_then(|uid| u32::try_from(uid).ok())
                    .and_then(|uid| Self::user_of(uid, &users)),
                health: identity::health_of(
                    matches!(process.status(), sysinfo::ProcessStatus::Zombie),
                    matches!(process.status(), sysinfo::ProcessStatus::Stop),
                    process.cpu_usage(),
                    process.memory(),
                ),
                container: None,
                exe_deleted,
                // macOS 无 Linux capabilities 语义（parity：平台专属字段恒空）。
                capabilities: Vec::new(),
            });
        }
        summaries.sort_by_key(|summary| summary.identity.pid());
        if summaries.is_empty() && !issues.is_empty() {
            return Inspection::failed(issues);
        }
        if issues.is_empty() {
            Inspection::complete(summaries)
        } else {
            Inspection::partial(summaries, issues)
        }
    }
}

/// 命令名（witr 显示名次序：exe 基名优先于 comm；sysinfo 的 name() 即
/// comm，空时回退 exe 基名）。
fn command_of(process: &sysinfo::Process) -> String {
    let name = process.name().to_string_lossy().trim().to_string();
    if !name.is_empty() {
        return name;
    }
    process
        .exe()
        .and_then(|path| path.file_name())
        .map_or_else(String::new, |base| base.to_string_lossy().into_owned())
}

/// 完整命令行（argv 单空格连接；不可得为 `None`）。
fn cmdline_of(process: &sysinfo::Process) -> Option<String> {
    let line = process
        .cmd()
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ");
    (!line.trim().is_empty()).then_some(line)
}