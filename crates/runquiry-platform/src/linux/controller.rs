//! Linux 进程控制：身份重读、pidfd 信号与受限优先级调整。

use std::io;
use std::os::fd::{FromRawFd, OwnedFd};

use runquiry_core::{
    CapabilityStatus, InspectError, Pid, ProcessAction, ProcessController, ProcessIdentity,
};

use super::LinuxPlatform;
use super::procfs::start_time_from_ticks;

impl ProcessController for LinuxPlatform {
    fn capability(&self) -> CapabilityStatus {
        if self.process_control_enabled() {
            CapabilityStatus::Supported
        } else {
            CapabilityStatus::Unsupported(String::from("注入式 /proc 平台禁止真实进程控制"))
        }
    }

    fn execute(
        &self,
        expected: &ProcessIdentity,
        action: ProcessAction,
    ) -> Result<(), InspectError> {
        if !self.process_control_enabled() {
            return Err(InspectError::Unsupported {
                reason: String::from("注入式 /proc 平台禁止真实进程控制"),
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

        // 先打开 pidfd：后续四类信号通过该句柄定向到同一进程对象，避免
        // 身份重读后 PID 被复用而把信号发给新进程。
        let pidfd = open_pidfd(raw_pid, expected.pid())?;
        let current = self.current_identity(expected.pid())?;
        if !same_control_target(expected, &current) {
            return Err(InspectError::ProcessChanged { identity: current });
        }

        match action {
            ProcessAction::Terminate => send_pidfd_signal(&pidfd, libc::SIGTERM, expected.pid()),
            ProcessAction::Kill => send_pidfd_signal(&pidfd, libc::SIGKILL, expected.pid()),
            ProcessAction::Pause => send_pidfd_signal(&pidfd, libc::SIGSTOP, expected.pid()),
            ProcessAction::Resume => send_pidfd_signal(&pidfd, libc::SIGCONT, expected.pid()),
            ProcessAction::Renice(value) => {
                send_pidfd_signal(&pidfd, 0, expected.pid())?;
                set_priority(expected.pid(), value.get())
            }
        }
    }
}

impl LinuxPlatform {
    fn current_identity(&self, pid: Pid) -> Result<ProcessIdentity, InspectError> {
        let stat = self
            .stat_of(pid.get())
            .map_err(|error| identity_read_error(pid, &error))?;
        let start_time = self
            .boot_time()
            .and_then(|boot| start_time_from_ticks(boot, stat.start_ticks));
        let executable = self.procfs.read_link(&format!("{}/exe", pid.get())).ok();
        Ok(ProcessIdentity::new(pid, start_time, executable))
    }
}

fn same_control_target(expected: &ProcessIdentity, current: &ProcessIdentity) -> bool {
    expected.same_process(current)
        && expected.executable().is_some()
        && expected.executable() == current.executable()
}

fn identity_read_error(pid: Pid, error: &io::Error) -> InspectError {
    match error.kind() {
        io::ErrorKind::NotFound => InspectError::NotFound {
            subject: format!("进程 {pid}"),
        },
        io::ErrorKind::PermissionDenied => InspectError::PermissionDenied {
            subject: format!("进程 {pid} 的身份"),
        },
        _ => InspectError::ProcessChanged {
            identity: ProcessIdentity::new(pid, None, None),
        },
    }
}

fn open_pidfd(raw_pid: libc::pid_t, pid: Pid) -> Result<OwnedFd, InspectError> {
    // SAFETY: [Category 8 - FFI boundary]. `SYS_pidfd_open` 接受值类型 PID 与
    // flags=0，不传入指针；raw_pid 已由受检转换得到。返回值在包装前检查为非负 fd。
    let result = unsafe { libc::syscall(libc::SYS_pidfd_open, raw_pid, 0_u32) };
    let fd = i32::try_from(result).map_err(|_| syscall_error("pidfd_open", pid))?;
    if fd < 0 {
        return Err(syscall_error("pidfd_open", pid));
    }
    // SAFETY: [Category 13 - library contract]. 成功的 pidfd_open 返回当前进程
    // 独占的新 fd；上方已验证非负，所有权在此唯一转交 OwnedFd 负责关闭。
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

fn send_pidfd_signal(pidfd: &OwnedFd, signal: i32, pid: Pid) -> Result<(), InspectError> {
    // SAFETY: [Category 8 - FFI boundary]. pidfd 由 pidfd_open 成功结果持有且存活；
    // siginfo 指针为空、flags=0 符合 pidfd_send_signal(2) 契约。
    let result = unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            std::os::fd::AsRawFd::as_raw_fd(pidfd),
            signal,
            std::ptr::null::<libc::siginfo_t>(),
            0_u32,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(syscall_error("pidfd_send_signal", pid))
    }
}

fn set_priority(pid: Pid, nice: i8) -> Result<(), InspectError> {
    // pidfd 不提供 setpriority 等价接口；调用前已用 pidfd signal=0 再确认存活，
    // 但仍存在该检查后、syscall 前退出并发生 PID 复用的内核级窄竞态。
    // SAFETY: [Category 8 - FFI boundary]. setpriority 仅接收值参数；PID 与 nice
    // 均已由领域类型限制在平台合法范围，不传入指针。
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
        Some(libc::ENOSYS) => InspectError::Unsupported {
            reason: format!("内核不支持 {operation}"),
        },
        _ => InspectError::Unsupported {
            reason: format!("{operation} 失败：{error}"),
        },
    }
}
