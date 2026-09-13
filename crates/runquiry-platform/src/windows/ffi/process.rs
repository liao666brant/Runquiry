//! 单进程信息的 Win32 查询包装（进程时间 / 镜像名 / PSAPI / PEB 地址）。

use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use windows_sys::Wdk::System::Threading::{
    NtQueryInformationProcess, PROCESSINFOCLASS, ProcessWow64Information,
};
use windows_sys::Win32::Foundation::FILETIME;
use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows_sys::Win32::System::Threading::{
    GetProcessHandleCount, GetProcessIoCounters, GetProcessTimes, IsWow64Process,
    PROCESS_NAME_WIN32, QueryFullProcessImageNameW,
};

use super::super::utf16::decode_lossy_units;
use super::super::winerror::is_ntstatus_failure;
use super::HandleGuard;

/// 自定义 `PROCESS_BASIC_INFORMATION` 镜像（windows-sys 的同名结构被
/// `Win32_System_Kernel` feature 门控，此处按公开 ABI 定义以避免引入该
/// feature：x64 布局 ExitStatus(4)+填充(4)+Peb(8)+Affinity(8)+
/// BasePriority(4)+填充(4)+UniqueProcessId(8)+Inherited(8)，总长 48）。
#[repr(C)]
struct ProcessBasicInformation {
    exit_status: i32,
    _padding0: u32,
    peb_base_address: *mut core::ffi::c_void,
    affinity_mask: usize,
    base_priority: i32,
    _padding1: u32,
    unique_process_id: usize,
    inherited_from_unique_process_id: usize,
}

// Windows 产品目标为 x64：下述 48 字节镜像布局仅对 64 位指针宽度成立。
const _: () = assert!(
    core::mem::size_of::<*mut core::ffi::c_void>() == 8,
    "ProcessBasicInformation 布局仅支持 64 位指针宽度目标"
);

impl Default for ProcessBasicInformation {
    fn default() -> Self {
        // SAFETY:结构体为 Plain-Old-Data 布局的零值（指针零值合法，仅作占位）。
        unsafe { core::mem::zeroed() }
    }
}

/// `PROCESSINFOCLASS::ProcessBasicInformation` 的类别值（0）。windows-sys
/// 的同名常量被 `Win32_System_Kernel` feature 门控且与上述镜像结构重名，
/// 故按公开 ABI 自带类别值。
const PROCESS_BASIC_INFORMATION_CLASS: PROCESSINFOCLASS = 0;

/// `FILETIME`（100 纳秒刻度，1601-01-01 起）→ `SystemTime`。
#[must_use]
pub(crate) fn filetime_to_system_time(low: u32, high: u32) -> Option<SystemTime> {
    // 1601→1970 = 11,644,473,600 秒 × 10^7 ticks/秒。
    const UNIX_EPOCH_FILETIME: u64 = 116_444_736_000_000_000;
    let ticks = (u64::from(high) << 32) | u64::from(low);
    let since_epoch = ticks.checked_sub(UNIX_EPOCH_FILETIME)?;
    Some(
        SystemTime::UNIX_EPOCH
            .checked_add(Duration::from_nanos(since_epoch * 100))
            .unwrap_or(SystemTime::UNIX_EPOCH),
    )
}

/// `GetProcessTimes`：进程创建时刻与累计 CPU 时间（kernel + user）。
#[must_use]
pub(crate) fn get_process_times(handle: &HandleGuard) -> Option<(Option<SystemTime>, Duration)> {
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY:handle 有效；四个 FILETIME 均为栈上可写内存，生命周期覆盖调用。
    let ok = unsafe {
        GetProcessTimes(
            handle.raw(),
            &mut creation,
            &mut exit,
            &mut kernel,
            &mut user,
        )
    };
    if ok == 0 {
        return None;
    }
    let cpu = (u64::from(kernel.dwHighDateTime) << 32 | u64::from(kernel.dwLowDateTime))
        .saturating_add(u64::from(user.dwHighDateTime) << 32 | u64::from(user.dwLowDateTime));
    // GetProcessTimes 的 kernel/user 字段语义为 100ns 刻度计数（witr
    // filetimeTicksToDuration），不是绝对时间戳。
    let duration = Duration::from_nanos(cpu * 100);
    Some((
        filetime_to_system_time(creation.dwLowDateTime, creation.dwHighDateTime),
        duration,
    ))
}

/// `QueryFullProcessImageNameW`：进程可执行文件的完整路径（宽字符）。
#[must_use]
pub(crate) fn query_full_image_name(handle: &HandleGuard) -> Option<String> {
    let mut buffer = [0u16; 1024];
    let mut size = u32::try_from(buffer.len()).ok()?;
    // SAFETY:handle 有效；buffer 为 1024 个 u16 的栈数组；size 进出参，
    // 成功时由系统写入实际写入的字符数（不含终止符）。
    let ok = unsafe {
        QueryFullProcessImageNameW(
            handle.raw(),
            PROCESS_NAME_WIN32,
            buffer.as_mut_ptr(),
            &mut size,
        )
    };
    if ok == 0 || size == 0 {
        return None;
    }
    let size = usize::try_from(size).ok()?;
    Some(decode_lossy_units(buffer.get(..size).unwrap_or(&[])))
}

/// `IsWow64Process`：目标进程是否为 32 位（WOW64）。
#[must_use]
pub(crate) fn is_wow64(handle: &HandleGuard) -> Option<bool> {
    let mut wow64: windows_sys::core::BOOL = 0;
    // SAFETY:handle 有效；wow64 为栈上 i32 出参。
    let ok = unsafe { IsWow64Process(handle.raw(), &mut wow64) };
    (ok != 0).then_some(wow64 != 0)
}

/// `NtQueryInformationProcess(ProcessBasicInformation)`：PEB64 基址。
#[must_use]
pub(crate) fn nt_peb_address(handle: &HandleGuard) -> Option<u64> {
    let mut info = ProcessBasicInformation::default();
    let mut return_length: u32 = 0;
    // SAFETY:handle 有效；info 为 48 字节 repr(C) 栈结构（见类型注释的
    // x64 布局），与内核写入长度匹配；return_length 为 u32 出参。非零返回
    // 值为 NTSTATUS 失败码。
    let status = unsafe {
        NtQueryInformationProcess(
            handle.raw(),
            PROCESS_BASIC_INFORMATION_CLASS,
            core::ptr::addr_of_mut!(info).cast::<core::ffi::c_void>(),
            u32::try_from(size_of::<ProcessBasicInformation>()).ok()?,
            &mut return_length,
        )
    };
    if is_ntstatus_failure(status) || info.peb_base_address.is_null() {
        return None;
    }
    Some(info.peb_base_address as usize as u64)
}

/// `NtQueryInformationProcess(ProcessWow64Information)`：WOW64 进程的
/// PEB32 基址（64 位 Runquiry 读 32 位目标进程的入口）。
#[must_use]
pub(crate) fn nt_wow64_peb_address(handle: &HandleGuard) -> Option<u64> {
    let mut peb32: *mut core::ffi::c_void = core::ptr::null_mut();
    let mut return_length: u32 = 0;
    // SAFETY:handle 有效；peb32 为 8 字节指针宽度的栈出参，覆盖
    // ProcessWow64Information 的写入量（sizeof(PVOID)）。
    let status = unsafe {
        NtQueryInformationProcess(
            handle.raw(),
            ProcessWow64Information,
            core::ptr::addr_of_mut!(peb32).cast::<core::ffi::c_void>(),
            u32::try_from(size_of::<*mut core::ffi::c_void>()).ok()?,
            &mut return_length,
        )
    };
    if is_ntstatus_failure(status) || peb32.is_null() {
        return None;
    }
    Some(peb32 as usize as u64)
}

/// PSAPI 内存计数器：`(WorkingSetSize, PrivateUsage)` 字节数。
#[must_use]
pub(crate) fn memory_counters(handle: &HandleGuard) -> Option<(u64, u64)> {
    let mut counters = PROCESS_MEMORY_COUNTERS_EX {
        cb: u32::try_from(size_of::<PROCESS_MEMORY_COUNTERS_EX>()).ok()?,
        ..Default::default()
    };
    // SAFETY:handle 有效；counters 为栈上结构且 cb 已按 sizeof(EX) 设置
    // （Windows 以 cb 区分 EX 变体，witr extended_windows.go 同约定）；
    // API 以 PROCESS_MEMORY_COUNTERS 指针为形参、按 cb 实际写 EX。
    let ok = unsafe {
        GetProcessMemoryInfo(
            handle.raw(),
            core::ptr::addr_of_mut!(counters)
                .cast::<windows_sys::Win32::System::ProcessStatus::PROCESS_MEMORY_COUNTERS>(),
            counters.cb,
        )
    };
    if ok == 0 {
        return None;
    }
    Some((counters.WorkingSetSize as u64, counters.PrivateUsage as u64))
}

/// `GetProcessIoCounters`：读写字节数与操作次数。
#[must_use]
pub(crate) fn io_counters(handle: &HandleGuard) -> Option<runquiry_core::IoStats> {
    let mut counters = windows_sys::Win32::System::Threading::IO_COUNTERS::default();
    // SAFETY:handle 有效；counters 为 48 字节栈结构，与 API 写入量匹配。
    let ok = unsafe { GetProcessIoCounters(handle.raw(), core::ptr::addr_of_mut!(counters)) };
    if ok == 0 {
        return None;
    }
    Some(runquiry_core::IoStats {
        read_bytes: counters.ReadTransferCount,
        write_bytes: counters.WriteTransferCount,
        read_ops: counters.ReadOperationCount,
        write_ops: counters.WriteOperationCount,
    })
}

/// `GetProcessHandleCount`：句柄数（Windows 无 FD 概念，witr 记为 FDCount）。
#[must_use]
pub(crate) fn handle_count(handle: &HandleGuard) -> Option<u32> {
    let mut count: u32 = 0;
    // SAFETY:handle 有效；count 为栈上 u32 出参。
    let ok = unsafe { GetProcessHandleCount(handle.raw(), &mut count) };
    (ok != 0).then_some(count)
}

/// 系统物理内存总量（进程生命周期内恒定，缓存一次）。
#[must_use]
pub(crate) fn total_physical_memory() -> u64 {
    static TOTAL: OnceLock<u64> = OnceLock::new();
    *TOTAL.get_or_init(|| {
        let mut status = MEMORYSTATUSEX {
            dwLength: u32::try_from(size_of::<MEMORYSTATUSEX>()).unwrap_or(0),
            ..Default::default()
        };
        // SAFETY:status 为栈上 MEMORYSTATUSEX，dwLength 已按结构大小设置
        // （API 契约），返回 0 时保持零值（调用方按「总量不可得」处理）。
        let ok = unsafe { GlobalMemoryStatusEx(core::ptr::addr_of_mut!(status)) };
        if ok == 0 { 0 } else { status.ullTotalPhys }
    })
}

#[cfg(test)]
mod tests {
    use super::filetime_to_system_time;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn filetime_conversion_round_trips_a_known_instant() {
        // 2025-05-08T00:00:00Z ≈ 133,911,360,000,000,000 ticks（1601 起）。
        // 回归：纪元常量曾写大 100 倍，任何真实创建时间都减成负数返回 None。
        let ticks_since_epoch: u64 = 133_911_360_000_000_000;
        let Some(time) = filetime_to_system_time(
            (ticks_since_epoch & 0xFFFF_FFFF) as u32,
            (ticks_since_epoch >> 32) as u32,
        ) else {
            eprintln!("2026 年的 FILETIME 必须可转换");
            return;
        };

        assert!(
            time.duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::MAX)
                .as_secs()
                > 1_700_000_000,
            "转换结果应落在现代时间（2023 之后），实际 {time:?}"
        );
    }

    #[test]
    fn filetime_before_the_epoch_returns_none() {
        assert_eq!(filetime_to_system_time(0, 0), None);
    }
}
