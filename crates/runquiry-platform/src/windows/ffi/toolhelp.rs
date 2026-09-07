//! Toolhelp32 快照枚举包装（进程全量列表与 sysinfo 失败时的回退）。

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};

use super::super::utf16::decode_fixed_array;
use super::super::winerror::{ERROR_NO_MORE_FILES, Win32Error};

/// ToolHelp32 快照条目（进程枚举与快照回退的轻量数据）。
#[derive(Debug, Clone)]
pub(crate) struct ToolhelpEntry {
    /// 进程 ID。
    pub pid: u32,
    /// 父进程 ID。
    pub parent_pid: u32,
    /// 可执行名（快照内 UTF-16 定长数组，截断到首个 NUL）。
    pub exe_name: String,
}

/// `CreateToolhelp32Snapshot` + `Process32FirstW/NextW` 全量枚举。
///
/// # Errors
/// 快照创建失败，或条目读取返回 `ERROR_NO_MORE_FILES` 之外的
/// `GetLastError` 时返回错误；空快照（无条目）返回空集合，不把真实错误
/// 伪装为空枚举。
pub(crate) fn toolhelp_snapshot() -> Result<Vec<ToolhelpEntry>, Win32Error> {
    // SAFETY:TH32CS_SNAPPROCESS 为系统快照标志；返回
    // INVALID_HANDLE_VALUE 表示失败，句柄所有权立即进入守卫语义（函数内
    // 显式 CloseHandle，成功路径结束时统一关闭）。
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE || snapshot.is_null() {
        // SAFETY:仅读取当前线程错误码；紧随失败的 CreateToolhelp32Snapshot，
        // 无中间 FFI 调用。
        let last_error = unsafe { GetLastError() };
        return Err(Win32Error(last_error));
    }
    let mut entries = Vec::new();
    let mut entry = PROCESSENTRY32W {
        dwSize: u32::try_from(size_of::<PROCESSENTRY32W>()).unwrap_or(0),
        ..Default::default()
    };
    // SAFETY:snapshot 为有效快照句柄；entry.dwSize 已按结构大小设置，
    // 系统按 dwSize 写入并保留未覆盖字段；返回 0 表示遍历结束或读取失败。
    let mut ok = unsafe { Process32FirstW(snapshot, core::ptr::addr_of_mut!(entry)) };
    while ok != 0 {
        entries.push(ToolhelpEntry {
            pid: entry.th32ProcessID,
            parent_pid: entry.th32ParentProcessID,
            exe_name: decode_fixed_array(&entry.szExeFile),
        });
        // SAFETY:同上；Process32NextW 读取失败（含遍历结束）时返回 0。
        ok = unsafe { Process32NextW(snapshot, core::ptr::addr_of_mut!(entry)) };
    }
    // 返回 0 的两种归因：ERROR_NO_MORE_FILES 是遍历结束（含空快照）的预期
    // 信号；其余错误不得伪装为空集合或静默截断。
    // SAFETY:仅读取当前线程错误码；紧随返回 0 的 Process32FirstW/NextW，
    // 无中间 FFI 调用。
    let last_error = unsafe { GetLastError() };
    let error = Win32Error(last_error);
    if error.0 != ERROR_NO_MORE_FILES {
        // SAFETY:snapshot 为本函数创建的快照句柄，此处为唯一关闭点。
        unsafe {
            CloseHandle(snapshot);
        }
        return Err(error);
    }
    // SAFETY:snapshot 为本函数创建的快照句柄，此处为唯一关闭点。
    unsafe {
        CloseHandle(snapshot);
    }
    Ok(entries)
}
