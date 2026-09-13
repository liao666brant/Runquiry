//! `ShellExecuteW` 安全包装：在系统文件管理器中定位文件（仅
//! `target_os = "windows"` 编译）。

use std::path::Path;

use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use super::super::reveal::{is_failure, select_parameter};

/// 以 NUL 终止的 UTF-16 缓冲。
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 在系统文件管理器中选中并展示 `path`（`explorer.exe /select,...`）。
///
/// 采用 `ShellExecuteW` 而非 `CommandRunner`：定位是「委托给 shell 的
/// fire-and-forget 展示」，不由本进程托管 Explorer。若经命令执行器启动，
/// 在 Explorer 未运行时该进程会常驻，命令执行器的超时回收将终止用户
/// shell——定位绝不允许这种副作用。
///
/// # Errors
/// `ShellExecuteW` 返回值不大于 32（`SE_ERR_*` / `ERROR_*`）时返回该码。
pub(crate) fn reveal_in_file_manager(path: &Path) -> Result<(), isize> {
    let verb = wide("open");
    let file = wide("explorer.exe");
    let parameters = wide(&select_parameter(path));
    // SAFETY:三个 PCWSTR 均指向以 NUL 终止的栈外 u16 缓冲（`wide` 产出），
    // 生命周期覆盖本次调用；hwnd 与 lpdirectory 显式为 null（无需父窗口、
    // 使用当前工作目录）；ShellExecuteW 仅读取这些参数，不接管所有权。
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            parameters.as_ptr(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    let code = result as isize;
    if is_failure(code) { Err(code) } else { Ok(()) }
}
