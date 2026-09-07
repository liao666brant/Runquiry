//! macOS 进程控制：身份重读 + kill(2) / setpriority（C1，B7 Linux
//! `controller.rs` 契约的 macOS 侧实现）。
//!
//! 与 Linux 的差异（已如实披露，不可用平台原语消除）：
//! * macOS 无 pidfd：TERM/KILL/STOP/CONT 按 PID 直接 `kill(2)`，**身份重读
//!   与发信号之间存在 PID 复用窗口**（启动时间粒度为秒，秒内复用不可区分）；
//! * renice：先 `kill(pid, 0)` 活性检查再 `setpriority`，与 Linux 相同的
//!   检查-执行窄 TOCTOU；
//! * start_time 来自 sysinfo（秒级）；不可得（0）→ same_process 恒 false →
//!   拒绝执行（保守拒绝，不产生副作用）。
//!
//! 权限不足 → [`InspectError::PermissionDenied`]；不自动提权。

use std::io;

use runquiry_core::{
    CapabilityStatus, InspectError, Pid, ProcessAction, ProcessController, ProcessIdentity,
};
use sysinfo::ProcessesToUpdate;

use super::MacosPlatform;
use super::identity::{same_control_target, start_time_from_unix_seconds};
use super::libproc;

impl ProcessController for MacosPlatform {
    fn capability(&self) -> CapabilityStatus {
        if self.process_control_enabled() {
            CapabilityStatus::Supported
        } else {
            CapabilityStatus::Unsupported(String::from("注入式实例禁止真实进程控制"))
        }
    }

    fn execute(
        &self,
        expected: &ProcessIdentity,
        action: ProcessAction,
    ) -> Result<(), InspectError> {
        if !self.process_control_enabled() {
            return Err(InspectError::Unsupported {
                reason: String::from("注入式实例禁止真实进程控制"),
            });
        }
        if expected.pid().get() == std::process::id() {
            return Err(InspectError::InvalidTarget {
                reason: String::from("拒绝控制 Runquiry 自身进程"),
            });
        }
        let raw_pid = libc::pid_t::try_from(expected.pid().get()).map_err(|_| {
            InspectError::InvalidTarget {
                reason: format!("PID {} 超出平台可表示范围", expected.pid()),
            }
        })?;
        // 身份重读（sysinfo 启动时间 + proc_pidpath）：比较不一致时零副作用
        // 返回 ProcessChanged。
        let current = self.current_identity(expected.pid())?;
        if !same_control_target(expected, &current) {
            return Err(InspectError::ProcessChanged { identity: current });
        }
        match action {
            ProcessAction::Terminate => send_signal(raw_pid, libc::SIGTERM, expected.pid()),
            ProcessAction::Kill => send_signal(raw_pid, libc::SIGKILL, expected.pid()),
            ProcessAction::Pause => send_signal(raw_pid, libc::SIGSTOP, expected.pid()),
            ProcessAction::Resume => send_signal(raw_pid, libc::SIGCONT, expected.pid()),
            ProcessAction::Renice(value) => {
                send_signal(raw_pid, 0, expected.pid())?;
                set_priority(expected.pid(), value.get())
            }
        }
    }
}

impl MacosPlatform {
    /// 重读当前身份：sysinfo 启动时间（秒级）+ `proc_pidpath` 可执行路径。
    fn current_identity(&self, pid: Pid) -> Result<ProcessIdentity, InspectError> {
        let mut system = sysinfo::System::new();
        system.refresh_processes(ProcessesToUpdate::Some(&[sysinfo::Pid::from_u32(
            pid.get(),
        )]));
        let process = system
            .process(sysinfo::Pid::from_u32(pid.get()))
            .ok_or_else(|| InspectError::NotFound {
                subject: format!("进程 {pid}"),
            })?;
        let executable = libproc::pid_path(pid)
            .ok()
            .flatten()
            .map(std::path::PathBuf::from);
        Ok(ProcessIdentity::new(
            pid,
            start_time_from_unix_seconds(process.start_time()),
            executable,
        ))
    }
}

/// kill(2) 单点信号（signal 0 = 活性检查，无副作用）。
fn send_signal(raw_pid: libc::pid_t, signal: libc::c_int, pid: Pid) -> Result<(), InspectError> {
    // SAFETY: [Category 8 - FFI boundary]。kill(2) 仅接收值类型 PID 与信号
    // 编号，不传入指针；raw_pid 已由受检转换得到。signal 0 仅探测活性。
    let result = unsafe { libc::kill(raw_pid, signal) };
    if result == 0 {
        Ok(())
    } else {
        Err(syscall_error("kill", pid))
    }
}

/// `setpriority(PRIO_PROCESS)`（与 Linux `set_priority` 同语义；pidfd 无
/// 等价接口，调用前的 signal 0 检查与 syscall 之间存在已披露的窄竞态）。
fn set_priority(pid: Pid, nice: i8) -> Result<(), InspectError> {
    // SAFETY: [Category 8 - FFI boundary]。setpriority 仅接收值参数；PID 与
    // nice 均已由领域类型限制在平台合法范围，不传入指针。
    let result = unsafe { libc::setpriority(libc::PRIO_PROCESS, pid.get(), i32::from(nice)) };
    if result == 0 {
        Ok(())
    } else {
        Err(syscall_error("setpriority", pid))
    }
}

fn syscall_error(operation: &str, pid: Pid) -> InspectError {
    let error = io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::ESRCH) => InspectError::NotFound {
            subject: format!("进程 {pid}"),
        },
        Some(code) if code == libc::EPERM || code == libc::EACCES => {
            InspectError::PermissionDenied {
                subject: format!("进程 {pid}"),
            }
        }
        _ => InspectError::Unsupported {
            reason: format!("{operation} 失败：{error}"),
        },
    }
}
