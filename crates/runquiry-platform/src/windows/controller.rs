//! Windows 进程控制：关闭类动作（terminate / kill / kill-tree）。
//!
//! Runquiry 扩展（parity §10 原整类 Unsupported 收窄为仅暂停/恢复/renice）：
//! 关闭类经 `TerminateProcess` 强杀；PID 复用防护沿用端口契约——**先比对
//! 创建时间、后终止**：目标以确认流程冻结的 expected 身份（PID +
//! `GetProcessTimes` 创建时间）比对，后代以快照 `start_time` 比对；身份
//! 不可验证一律拒绝/跳过，绝不误杀复用后的新进程。暂停/恢复/renice 在
//! Windows 不提供（ntdll 未文档化 API / 优先级类有损映射，v1 不引入），
//! 经 [`ProcessController::action_capability`] 表达。

use std::time::SystemTime;

use runquiry_core::{
    CapabilityStatus, HealthStatus, InspectError, Pid, ProcessAction, ProcessController,
    ProcessIdentity, ProcessSummary, collect_descendants,
};
use windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED;
use windows_sys::Win32::System::Threading::{
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, TerminateProcess,
};

use super::WindowsPlatform;
use super::ffi;
use super::ffi::HandleGuard;
use super::process_list::{start_time_from_sysinfo, sysinfo_snapshot};
use super::reveal::{RevealFailure, failure_kind};
use super::unsupported::{KILL_ONLY_REASON, unsupported_action_error};

impl ProcessController for WindowsPlatform {
    fn capability(&self) -> CapabilityStatus {
        CapabilityStatus::Supported
    }

    fn action_capability(&self, action: &ProcessAction) -> CapabilityStatus {
        match action {
            ProcessAction::Terminate | ProcessAction::Kill | ProcessAction::KillTree => {
                CapabilityStatus::Supported
            }
            ProcessAction::Pause | ProcessAction::Resume | ProcessAction::Renice(_) => {
                CapabilityStatus::Unsupported(String::from(KILL_ONLY_REASON))
            }
        }
    }

    /// 可执行文件定位：Windows 经 `ShellExecuteW` 委托系统文件管理器
    /// （`explorer.exe /select`）展示，属平台能力，恒可用。
    fn reveal_capability(&self) -> CapabilityStatus {
        CapabilityStatus::Supported
    }

    /// 在资源管理器中选中该进程的可执行文件。
    ///
    /// 只读展示，不做破坏性动作前的身份重读比对：优先以实时路径
    /// （`QueryFullProcessImageNameW`，回退 sysinfo）定位，均不可得时回退
    /// 确认流程持有的快照路径；两者皆无 → `NotFound`。文件管理器失败按
    /// `ShellExecuteW` 错误码区分「路径不可得 / 拒绝访问 / 调用失败」，不把
    /// 运行期失败说成平台能力缺失。
    fn reveal_executable(&self, identity: &ProcessIdentity) -> Result<(), InspectError> {
        let pid = identity.pid();
        let path = live_executable(pid.get())
            .or_else(|| identity.executable().cloned())
            .ok_or_else(|| InspectError::NotFound {
                subject: format!("进程 {pid} 的可执行文件路径"),
            })?;
        ffi::reveal_in_file_manager(&path).map_err(|code| match failure_kind(code) {
            RevealFailure::Missing => InspectError::NotFound {
                subject: format!("可执行文件 {}", path.display()),
            },
            RevealFailure::AccessDenied => InspectError::PermissionDenied {
                subject: format!("进程 {pid} 的可执行文件 {}", path.display()),
            },
            RevealFailure::Other => InspectError::ExternalTool {
                program: String::from("explorer.exe"),
                detail: format!("ShellExecuteW 错误码 {code}"),
            },
        })
    }

    fn execute(
        &self,
        expected: &ProcessIdentity,
        action: ProcessAction,
    ) -> Result<(), InspectError> {
        if expected.pid().get() == std::process::id() {
            return Err(InspectError::InvalidTarget {
                reason: String::from("拒绝控制 Runquiry 自身进程"),
            });
        }
        let Some(expected_start) = expected.start_time() else {
            // expected 身份不可验证：端口契约要求无副作用地拒绝。
            return Err(InspectError::ProcessChanged {
                identity: ProcessIdentity::new(expected.pid(), None, None),
            });
        };
        match action {
            ProcessAction::Terminate | ProcessAction::Kill => {
                kill_verified(expected.pid(), expected_start)
            }
            // KillTree：目标先死，后代随后按快照广度顺序尽力逐个清杀。
            ProcessAction::KillTree => {
                kill_verified(expected.pid(), expected_start)?;
                self.kill_descendants(expected.pid())
            }
            // 正常流程不可达：action_capability 已将三类动作标记 Unsupported，
            // UI 不得渲染入口；防御性返回保证误用不产生副作用。
            ProcessAction::Pause | ProcessAction::Resume | ProcessAction::Renice(_) => {
                Err(unsupported_action_error())
            }
        }
    }
}

impl WindowsPlatform {
    /// KillTree 的后代清杀：实时快照收集后代（广度优先），逐个「开
    /// `PROCESS_TERMINATE` 句柄 → 创建时间比对快照 `start_time` → 终止」。
    /// 后代已退出或身份不可验证（创建时间缺失/不匹配，含 PID 复用）一律
    /// 跳过；权限等其余失败在全部尝试后返回首个错误。后代中出现 Runquiry
    /// 自身则在任何后代被杀前整体拒绝（不做部分清杀；目标本身已先行强杀）。
    fn kill_descendants(&self, target: Pid) -> Result<(), InspectError> {
        let entries = descendant_snapshot();
        let descendants = collect_descendants(target, &entries);
        let self_pid = std::process::id();
        if descendants
            .iter()
            .any(|descendant| descendant.identity.pid().get() == self_pid)
        {
            return Err(InspectError::InvalidTarget {
                reason: String::from("目标进程的后代包含 Runquiry 自身，拒绝执行进程树清杀"),
            });
        }
        let mut first_error: Option<InspectError> = None;
        for descendant in descendants {
            let pid = descendant.identity.pid();
            let Some(expected_start) = descendant.identity.start_time() else {
                continue; // 快照未取得启动时间：身份不可验证，跳过
            };
            if let Err(error) = kill_descendant(pid, expected_start)
                && !matches!(error, InspectError::NotFound { .. })
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

/// 目标进程当前的可执行文件路径（实时）：`QueryFullProcessImageNameW` 优先，
/// 回退 sysinfo。进程不可打开/已退出时为 `None`。
fn live_executable(pid: u32) -> Option<std::path::PathBuf> {
    if let Ok(handle) = HandleGuard::open_process(PROCESS_QUERY_LIMITED_INFORMATION, pid)
        && let Some(path) = ffi::query_full_image_name(&handle)
    {
        return Some(std::path::PathBuf::from(path));
    }
    let system = sysinfo_snapshot();
    system
        .process(sysinfo::Pid::from_u32(pid))
        .and_then(|process| process.exe().map(std::path::Path::to_path_buf))
}

/// KillTree 后代的实时轻量快照（仅 PID/PPID/start_time 参与树收集与防护）。
fn descendant_snapshot() -> Vec<ProcessSummary> {
    let system = sysinfo_snapshot();
    system
        .processes()
        .iter()
        .map(|(pid, process)| ProcessSummary {
            identity: ProcessIdentity::new(
                Pid::new(pid.as_u32()).unwrap_or(Pid::MIN),
                start_time_from_sysinfo(process.start_time()),
                None,
            ),
            parent_pid: process
                .parent()
                .and_then(|parent| Pid::new(parent.as_u32()).ok()),
            command: String::new(),
            command_line: None,
            user: None,
            health: HealthStatus::Unknown,
            container: None,
            exe_deleted: false,
            capabilities: Vec::new(),
            cpu_time_seconds: None,
            cpu_percent: None,
            memory_rss_bytes: None,
            memory_percent: None,
        })
        .collect()
}

/// 确认流程目标：身份比对通过后强杀；不可验证/已消失/拒绝按端口契约报错。
///
/// # Errors
/// 创建时间与期望不一致 → `ProcessChanged`（无副作用）；打开句柄拒绝 →
/// `PermissionDenied`、进程不存在 → `NotFound`；终止调用失败 → `Unsupported`。
fn kill_verified(pid: Pid, expected_start: SystemTime) -> Result<(), InspectError> {
    let handle = HandleGuard::open_process(
        PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
        pid.get(),
    )
    .map_err(|error| open_error(pid, error))?;
    // PID 复用防护：创建时间必须在终止**之前**比对——句柄虽指向打开时刻的
    // 进程对象，但若该对象已非 expected 身份（复用窗口），此刻终止即误杀。
    let Some(creation) = ffi::get_process_times(&handle).and_then(|(creation, _)| creation) else {
        return Err(InspectError::ProcessChanged {
            identity: ProcessIdentity::new(pid, None, None),
        });
    };
    if !same_start_second(creation, expected_start) {
        return Err(InspectError::ProcessChanged {
            identity: ProcessIdentity::new(pid, Some(creation), None),
        });
    }
    // SAFETY:handle 来自 OpenProcess 成功结果且在 HandleGuard 存活期内；
    // TerminateProcess 仅接收句柄与退出码标量。返回 0 表示失败。
    let ok = unsafe { TerminateProcess(handle.raw(), 1) };
    if ok != 0 {
        return Ok(());
    }
    // SAFETY:紧随失败的 TerminateProcess，无中间 FFI 调用。
    let last_error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
    if last_error == ERROR_ACCESS_DENIED {
        Err(InspectError::PermissionDenied {
            subject: format!("进程 {pid}"),
        })
    } else {
        Err(InspectError::Unsupported {
            reason: format!("TerminateProcess 失败：Win32 错误码 {last_error}"),
        })
    }
}

/// KillTree 后代的单进程清杀：与 [`kill_verified`] 同序，但语义按「尽力」
/// 折叠——进程已消失或身份不可验证一律按跳过处理（`Ok`），仅权限/调用失败
/// 上抛供聚合。
///
/// # Errors
/// 仅权限拒绝（`PermissionDenied`）与终止调用失败（`Unsupported`）。
fn kill_descendant(pid: Pid, expected_start: SystemTime) -> Result<(), InspectError> {
    let handle = match HandleGuard::open_process(
        PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
        pid.get(),
    ) {
        Ok(handle) => handle,
        // 权限拒绝（受保护后代）不得静默吞掉：聚合上抛，UI 不把存活后代报成已清杀。
        Err(error) if error.is_access_denied() => {
            return Err(InspectError::PermissionDenied {
                subject: format!("进程 {pid}"),
            });
        }
        // 进程已消失/参数非法：按已清杀处理（端口契约的「已退出」语义）。
        Err(error) if error.is_target_gone_or_invalid() => return Ok(()),
        // 其余未归类失败同样聚合上抛，不伪装成「已退出」。
        Err(error) => {
            return Err(InspectError::Unsupported {
                reason: format!("进程 {pid} 打开失败：Win32 错误码 {}", error.0),
            });
        }
    };
    let Some(creation) = ffi::get_process_times(&handle).and_then(|(creation, _)| creation) else {
        return Ok(()); // 创建时间不可得：身份不可验证，跳过
    };
    if !same_start_second(creation, expected_start) {
        return Ok(()); // PID 已复用：绝不误杀新进程
    }
    // SAFETY:handle 来自 OpenProcess 成功结果且在 HandleGuard 存活期内。
    let ok = unsafe { TerminateProcess(handle.raw(), 1) };
    if ok != 0 {
        return Ok(());
    }
    // SAFETY:紧随失败的 TerminateProcess，无中间 FFI 调用。
    let last_error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
    if last_error == ERROR_ACCESS_DENIED {
        Err(InspectError::PermissionDenied {
            subject: format!("进程 {pid}"),
        })
    } else {
        Err(InspectError::Unsupported {
            reason: format!("TerminateProcess 失败：Win32 错误码 {last_error}"),
        })
    }
}

/// OpenProcess 失败映射（目标路径）：拒绝 → `PermissionDenied`；其余
/// （进程已消失的无效参数等）→ `NotFound`。
fn open_error(pid: Pid, error: super::winerror::Win32Error) -> InspectError {
    if error.0 == ERROR_ACCESS_DENIED {
        InspectError::PermissionDenied {
            subject: format!("进程 {pid}"),
        }
    } else {
        InspectError::NotFound {
            subject: format!("进程 {pid}"),
        }
    }
}

/// 真机活测试：真实 spawn 进程并经 controller 强杀（只读之外的真实控制
/// 路径证据；目标为本测试自己启动的短命进程，不影响系统）。
#[cfg(all(test, target_os = "windows"))]
mod live {
    use super::{
        HandleGuard, Pid, ProcessAction, ProcessController, ProcessIdentity, WindowsPlatform, ffi,
    };
    use windows_sys::Win32::System::Threading::PROCESS_QUERY_INFORMATION;

    /// 取真实创建时间构造 expected 身份（与生产确认流同一防护基准）。
    fn identity_of(pid: u32) -> Result<ProcessIdentity, Box<dyn std::error::Error>> {
        let handle = HandleGuard::open_process(PROCESS_QUERY_INFORMATION, pid)
            .map_err(|error| format!("打开 PID {pid} 失败：Win32 错误码 {}", error.0))?;
        let times = ffi::get_process_times(&handle);
        let Some(Some(creation)) = times.map(|(creation, _)| creation) else {
            return Err(format!("PID {pid} 创建时间不可得（ffi={times:?}）").into());
        };
        Ok(ProcessIdentity::new(
            Pid::new(pid).map_err(|_| "PID 非法")?,
            Some(creation),
            None,
        ))
    }

    #[test]
    fn reveal_capability_is_supported_and_resolves_a_real_executable()
    -> Result<(), Box<dyn std::error::Error>> {
        let platform = WindowsPlatform::new()?;
        // 能力恒 Supported（Windows 委托系统文件管理器）。
        assert!(ProcessController::reveal_capability(&platform).is_usable());

        // 只验证路径解析（不触发 ShellExecuteW 以免弹出资源管理器窗口）：
        // 以本测试进程自身 PID 解析出真实存在的 .exe 路径。
        let own = std::process::id();
        let resolved = super::live_executable(own);
        let path = resolved.ok_or("应能解析当前进程的可执行路径")?;
        assert!(path.exists(), "解析出的路径应真实存在：{}", path.display());
        Ok(())
    }

    #[test]
    fn kill_terminates_a_real_process() -> Result<(), Box<dyn std::error::Error>> {
        let mut child = std::process::Command::new("cmd")
            .args(["/c", "ping -n 30 127.0.0.1 > nul"])
            .spawn()?;
        let pid = child.id();
        let identity = identity_of(pid)?;
        std::thread::sleep(std::time::Duration::from_millis(300));
        let platform = WindowsPlatform::new()?;

        ProcessController::execute(&platform, &identity, ProcessAction::Kill)?;

        let status = child.wait()?;
        assert!(!status.success(), "被强杀进程不应以成功码退出");
        Ok(())
    }

    #[test]
    fn kill_tree_terminates_target_and_descendants() -> Result<(), Box<dyn std::error::Error>> {
        let mut child = std::process::Command::new("cmd")
            .args(["/c", "ping -n 60 127.0.0.1 > nul"])
            .spawn()?;
        let cmd_pid = child.id();
        let identity = identity_of(cmd_pid)?;
        let platform = WindowsPlatform::new()?;

        // 确认 cmd 确实派生了 ping 子进程，并记录其存活证据。
        let descendant_alive = |target: u32| -> bool {
            let mut system = sysinfo::System::new();
            system.refresh_processes(sysinfo::ProcessesToUpdate::All);
            system.processes().values().any(|process| {
                process
                    .parent()
                    .is_some_and(|parent| parent.as_u32() == target)
            })
        };
        assert!(descendant_alive(cmd_pid), "测试前提：cmd 应已派生子进程");

        ProcessController::execute(&platform, &identity, ProcessAction::KillTree)?;

        child.wait()?;
        assert!(
            !descendant_alive(cmd_pid),
            "KillTree 后 cmd 的子进程应全部退出"
        );
        Ok(())
    }
}

/// 启动时间按秒对齐比较：快照身份的 `start_time` 来自 sysinfo 的整秒精度，
/// `GetProcessTimes` 创建时间为 100ns 精度；整秒相等即视为同一进程
/// （残留窄竞态：同一秒内且同 PID 的完整复用窗口，与 Linux renice 的
/// 既有文档化残差同级）。
fn same_start_second(creation: SystemTime, expected: SystemTime) -> bool {
    let secs = |time: SystemTime| {
        time.duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .map(|duration| duration.as_secs())
    };
    secs(creation) == secs(expected)
}
