//! macOS 平台适配器（C1，模块 06）。
//!
//! 仅在 `target_os = "macos"` 下参与编译（lib.rs cfg 导出）。纯解析逻辑
//! （lsof / launchctl / plist / 身份比对）放在无 OS 依赖的兄弟模块
//! （[`lsof`] / [`launchctl`] / [`plist`] / [`identity`]），集成测试以
//! `#[path]` 在 Linux 直接编译；本文件与 process / details / network /
//! files / source / controller / container / libproc 等 OS 模块在 Linux
//! 不参与编译。
//!
//! [`LinuxPlatform`](crate::linux::LinuxPlatform) 的部分成功语义在此对齐：
//! 单点失败只追加 [`runquiry_core::DiagnosticIssue`]，不丢条目、不返回空
//! 集合冒充成功；全部外部命令（lsof / launchctl / plutil / ps）经
//! [`StdCommandRunner`](crate::command::StdCommandRunner)，不经过 shell。

use std::collections::HashMap;
use std::time::Duration;

use runquiry_core::{CommandOutput, CommandRunner as _, CommandSpec, InspectError};
use sysinfo::{ProcessesToUpdate, System};

mod container;
mod controller;
mod details;
mod files;
mod libproc;
mod network;
mod process;
mod source;

pub(crate) mod identity;
pub(crate) mod launchctl;
pub(crate) mod lsof;
pub(crate) mod plist;

pub use process::MacosPlatform;

/// 构造时的 PID 基线快照（sysinfo 全量枚举）。
pub(super) fn sysinfo_pids() -> Vec<u32> {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All);
    let mut pids: Vec<u32> = system.processes().keys().map(|pid| pid.as_u32()).collect();
    pids.sort_unstable();
    pids
}

/// 全量快照的 PPID 映射（自身后代排除与子进程发现共用）。
pub(super) fn ppid_map(system: &System) -> HashMap<u32, u32> {
    system
        .processes()
        .values()
        .filter_map(|process| {
            let parent = process.parent()?;
            Some((process.pid().as_u32(), parent.as_u32()))
        })
        .collect()
}

impl MacosPlatform {
    /// 单次全量 sysinfo 快照（进程基线、详情与子进程发现共用）。
    pub(super) fn snapshot(&self) -> System {
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::All);
        system
    }

    /// 经受限 `CommandRunner` 执行一次外部命令。
    ///
    /// # Errors
    /// 程序缺失、超时或输出超限返回 [`InspectError::ExternalTool`]（不返回
    /// 半截结果）；非零退出属正常返回（exit_code 保留在输出内）。
    pub(super) fn run(
        &self,
        spec: &CommandSpec,
        timeout: Duration,
    ) -> Result<CommandOutput, InspectError> {
        self.runner.run(spec, timeout)
    }

    /// 执行一次 `lsof`（Runquiry 的网络/文件/详情采集统一 argv 入口）。
    ///
    /// # Errors
    /// 同 [`Self::run`]：缺失/超时/超限 → [`InspectError::ExternalTool`]。
    pub(super) fn run_lsof(
        &self,
        args: &[&str],
        timeout: Duration,
    ) -> Result<CommandOutput, InspectError> {
        self.run(&CommandSpec::new("lsof", args.iter().copied()), timeout)
    }

    /// 是否应从基线排除（Linux `LinuxPlatform::excluded` 同语义）：
    /// 自身；基准快照之后新出现且（a）PPID 链可达自身，或（b）启动时刻晚于
    /// 构造时刻（秒级粒度，收养后代的时间窗兜底）。
    pub(super) fn excluded(
        &self,
        pid: u32,
        ppids: &HashMap<u32, u32>,
        start_seconds: Option<u64>,
    ) -> bool {
        if pid == self.own_pid.get() {
            return true;
        }
        if self.baseline_pids.contains(&pid) {
            return false;
        }
        if let Some(construction) = self.constructed_at
            && let Some(seconds) = start_seconds
            && let Some(start) = identity::start_time_from_unix_seconds(seconds)
            && start > construction
        {
            return true;
        }
        // 沿 PPID 链上溯找自身；环与超长链以步数上限截断。
        let mut current = pid;
        for _ in 0..4_096 {
            match ppids.get(&current) {
                Some(&parent) if parent == self.own_pid.get() => return true,
                Some(&parent) if parent != 0 => current = parent,
                _ => return false,
            }
        }
        false
    }

    /// 进程控制副作用开关：仅生产构造（构造时刻为墙钟）允许。
    pub(super) const fn process_control_enabled(&self) -> bool {
        self.constructed_at.is_some()
    }
}

/// 非零退出但有 stdout 的抢救诊断（witr cwd/txt / lsof 全量扫描的退出码
/// 抢救语义；退出码保留，条目由调用方解析 stdout 保留）。
pub(super) fn exit_salvage_issue(
    output: &CommandOutput,
    command: &str,
) -> Option<runquiry_core::DiagnosticIssue> {
    match output.exit_code {
        Some(0) | None => None,
        Some(code) => Some(runquiry_core::DiagnosticIssue::new(
            runquiry_core::DiagnosticCode::ExternalToolFailed,
            format!("{command} 非零退出（{code}），已按部分 stdout 抢救"),
        )),
    }
}