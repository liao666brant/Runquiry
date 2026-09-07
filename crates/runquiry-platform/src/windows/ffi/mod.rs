//! Windows API 安全包装（仅 `target_os = "windows"` 编译）。
//!
//! 本 crate 的全部 FFI 调用集中在本目录（`ffi` / `ffi_scm`）与
//! `network` / `source` 的采集入口：句柄经 [`HandleGuard`] RAII 关闭，
//! 远程内存读取有界并校验实际读取字节数。每个 `unsafe` 块的 SAFETY 注释
//! 说明指针、长度、对齐与所有权依据。所需 ntdll 函数由 windows-sys 的
//! `Wdk_System_Threading` feature 导出，本目录不包含手写 extern。
//!
//! 结构：本文件（句柄守卫与远程内存读取）、[`process`](self::process)
//! （单进程信息查询与 PEB 地址）、[`toolhelp`](self::toolhelp)（快照枚举）、
//! `ffi_scm`（SCM 枚举与配置查询）。

mod process;
mod toolhelp;

pub(super) use process::{
    get_process_times, handle_count, io_counters, is_wow64, memory_counters, nt_peb_address,
    nt_wow64_peb_address, query_full_image_name, total_physical_memory,
};
pub(super) use toolhelp::toolhelp_snapshot;

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
use windows_sys::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ,
};

use super::winerror::Win32Error;

/// 进程 / 服务句柄的 RAII 守卫：析构时统一 `CloseHandle` /
/// `CloseServiceHandle`，调用方无需手工释放。
pub(super) struct HandleGuard(HANDLE);

impl HandleGuard {
    /// 以请求的访问权限打开进程（`inherit = false`）。
    ///
    /// # Errors
    /// 返回 0 句柄（无权限 / 进程已消失 / 参数非法）时返回 `GetLastError`。
    pub(super) fn open_process(access: u32, pid: u32) -> Result<Self, Win32Error> {
        // SAFETY:OpenProcess 仅读取标量参数；返回值 0 表示失败（HANDLE
        // 非空指针即有效），错误码经 GetLastError 取得。句柄所有权立即移入
        // HandleGuard，析构时 CloseHandle。
        let handle = unsafe { OpenProcess(access, 0, pid) };
        if handle.is_null() {
            // SAFETY:仅读取当前线程错误码；紧随失败的 OpenProcess，无中间
            // FFI 调用。
            let last_error = unsafe { GetLastError() };
            return Err(Win32Error(last_error));
        }
        Ok(Self(handle))
    }

    /// 原始句柄（只读借用；调用方不得关闭或复制所有权）。
    #[must_use]
    pub(super) const fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for HandleGuard {
    fn drop(&mut self) {
        // SAFETY:self.0 来自 OpenProcess 成功返回且未被复制或提前关闭；
        // Drop 恰好执行一次。
        unsafe {
            CloseHandle(self.0);
        }
    }
}

/// 读取远端进程内存（有界：`out.len()` 为上限，实际读取必须等于请求量）。
///
/// # Errors
/// 读取失败或实际读取字节数不足（进程在读取中退出 / 页面不可读）时返回
/// `GetLastError` 或 0（以 `ERROR_INVALID_HANDLE` 归一，调用方按部分结果处理）。
pub(super) fn read_remote_memory(
    handle: &HandleGuard,
    address: u64,
    out: &mut [u8],
) -> Result<(), Win32Error> {
    if out.is_empty() {
        return Ok(());
    }
    let mut bytes_read: usize = 0;
    // SAFETY:handle 为 OpenProcess 返回的有效句柄；address 是目标进程内
    // 经 NtQueryInformationProcess / PEB 计划得出的地址；out 的指针与长度
    // 由切片保证，生命周期覆盖本次调用；bytes_read 为 8 字节 usize（witr
    // 注释强调必须按指针宽度传递，不得用 u32）。返回 0 视为失败。
    let ok = unsafe {
        ReadProcessMemory(
            handle.raw(),
            address as *const core::ffi::c_void,
            out.as_mut_ptr().cast::<core::ffi::c_void>(),
            out.len(),
            &mut bytes_read,
        )
    };
    if ok == 0 {
        // SAFETY:仅读取当前线程错误码；紧随失败的 ReadProcessMemory，无
        // 中间 FFI 调用。
        let last_error = unsafe { GetLastError() };
        return Err(Win32Error(last_error));
    }
    if bytes_read != out.len() {
        // 部分读取（进程退出 / PEB 变化）：不返回半截数据当作完整。
        return Err(Win32Error(super::winerror::ERROR_INVALID_HANDLE));
    }
    Ok(())
}

/// 分级打开进程：先 `QUERY_INFORMATION | VM_READ`，权限不足降级
/// `QUERY_LIMITED_INFORMATION`；两级都失败返回最后一次错误。
///
/// # Errors
/// 两次 OpenProcess 均失败时返回最后一次 `GetLastError`。
pub(super) fn open_process_graded(pid: u32) -> Result<HandleGuard, Win32Error> {
    match HandleGuard::open_process(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, pid) {
        Ok(handle) => Ok(handle),
        Err(denied) => {
            let fallback = HandleGuard::open_process(PROCESS_QUERY_LIMITED_INFORMATION, pid);
            fallback.map_err(|limited| {
                // 访问拒绝优先（信息量更大）；参数非法（进程消失）次之。
                if limited.is_target_gone_or_invalid() {
                    limited
                } else {
                    denied
                }
            })
        }
    }
}
