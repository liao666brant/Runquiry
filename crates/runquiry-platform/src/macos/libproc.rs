//! libproc 安全包装（C1）：全部 libproc FFI 收敛于本模块。
//!
//! 布局依据（Apple SDK，交叉编译检查将在 macOS 上核对）：
//! * `sys/proc_info.h`：`struct proc_taskinfo`（18 字段，96 字节，
//!   `PROC_PIDTASKINFO_SIZE = 96`）、`PROC_PIDLISTFDS = 1` 的
//!   `struct proc_fdinfo { int proc_fd; uint32_t proc_fdtype; }`（8 字节）；
//! * `sys/resource.h`：`rusage_info_v4`（flavor `RUSAGE_INFO_V4 = 4`）。
//!
//! `rusage_info_v4` 刻意**不按结构体声明**，而以 1 KiB 8 字节对齐零填充缓冲
//! 承接：SDK 侧结构即使大于任何 Rust 侧误声明也不会越界写；只按声明偏移
//! 读取 `ri_diskio_bytesread`（offset 128）与 `ri_diskio_byteswritten`
//! （offset 136）。偏移推算：`ri_uuid[16]`（0..15）后为连续 u64 字段，至
//! `ri_child_pageins`@112、`ri_child_elapsed_abstime`@120、
//! `ri_diskio_bytesread`@128、`ri_diskio_byteswritten`@136，
//! `sizeof(rusage_info_v4) == 144`（Apple SDK `sys/resource.h` 的
//! `rusage_info_v4` 全量字段，witr `libproc_darwin_cgo.go` 经
//! `RUSAGE_INFO_V4` 消费同一布局）。

use std::io;

use runquiry_core::IoStats;

/// `sys/proc_info.h`：`PROC_PIDLISTFDS`。
const PROC_PIDLISTFDS: libc::c_int = 1;
/// `sys/proc_info.h`：`PROC_PIDTASKINFO`。
const PROC_PIDTASKINFO: libc::c_int = 4;
/// `sys/proc_info.h`：`PROC_PIDTASKINFO_SIZE`（布局断言见 `ProcTaskInfo`）。
const PROC_PIDTASKINFO_SIZE: libc::c_int = 96;
/// `sys/resource.h`：`RUSAGE_INFO_V4`。
const RUSAGE_INFO_V4: libc::c_int = 4;
/// `sys/proc_info.h`：`PROC_PIDPATHINFO_MAXSIZE = 4 * MAXPATHLEN`。
const PROC_PIDPATHINFO_MAXSIZE: libc::c_uint = 4 * 1024;
/// `rusage_info_v4` 内 `ri_diskio_bytesread` 的字节偏移（见模块文档）。
const RUSAGE_DISKIO_BYTESREAD_OFFSET: usize = 128;
/// `rusage_info_v4` 内 `ri_diskio_byteswritten` 的字节偏移（见模块文档）。
const RUSAGE_DISKIO_BYTESWRITTEN_OFFSET: usize = 136;

unsafe extern "C" {
    /// `libproc.h`：`int proc_pidpath(int pid, void *buffer, uint32_t size)`。
    fn proc_pidpath(
        pid: libc::c_int,
        buffer: *mut libc::c_void,
        buffersize: libc::c_uint,
    ) -> libc::c_int;
    /// `libproc.h`：`int proc_pidinfo(int pid, int flavor, uint64_t arg,
    /// void *buffer, int size)`。
    fn proc_pidinfo(
        pid: libc::c_int,
        flavor: libc::c_int,
        arg: u64,
        buffer: *mut libc::c_void,
        buffersize: libc::c_int,
    ) -> libc::c_int;
    /// `libproc.h`：`int proc_pid_rusage(int pid, int flavor, void *buffer)`。
    fn proc_pid_rusage(pid: libc::c_int, flavor: libc::c_int, buffer: *mut libc::c_void)
    -> libc::c_int;
}

/// `sys/proc_info.h` 的 `struct proc_taskinfo`（字段与顺序逐一对齐；前六个
/// 计数/尺寸字段与 `pti_threads_user` / `pti_threads_system` 均为 `uint64_t`，
/// 其余为 `int32_t`）。
#[repr(C)]
struct ProcTaskInfo {
    pti_virtual_size: u64,
    pti_resident_size: u64,
    pti_total_user: u64,
    pti_total_system: u64,
    pti_threads_user: u64,
    pti_threads_system: u64,
    pti_policy: i32,
    pti_faults: i32,
    pti_pageins: i32,
    pti_cow_faults: i32,
    pti_messages_sent: i32,
    pti_messages_received: i32,
    pti_syscalls_mach: i32,
    pti_syscalls_unix: i32,
    pti_csw: i32,
    pti_threadnum: i32,
    pti_numrunning: i32,
    pti_priority: i32,
}

// 布局断言：proc_taskinfo 必须恰为 PROC_PIDTASKINFO_SIZE（96 字节）。
const _: () = assert!(size_of::<ProcTaskInfo>() == 96);

/// 单次 `proc_pidinfo(PROC_PIDTASKINFO)` 的消费字段。
#[derive(Debug, Clone, Copy)]
pub(crate) struct TaskSnapshot {
    /// 虚拟内存（字节）。
    pub virtual_bytes: u64,
    /// 常驻内存（字节）。
    pub resident_bytes: u64,
    /// 线程数（core 模型暂无承载字段，仅保留在平台侧快照）。
    pub threads: i32,
}

/// 进程可执行文件路径（witr 以 lsof `txt` 行为 exe 来源；此处用
/// `proc_pidpath`，属披露的等价替换——同为内核权威路径）。失败（进程已
/// 退出、权限不足或路径不可得）返回 `Ok(None)`。
pub(crate) fn pid_path(pid: u32) -> io::Result<Option<String>> {
    let raw_pid = to_c_int(pid);
    // 字节缓冲（proc_pidpath 以 *mut c_void 接收，内容按路径字节语义消费）。
    let mut buffer = [0u8; PROC_PIDPATHINFO_MAXSIZE as usize];
    // SAFETY: [Category 8 - FFI boundary]。buffer 为本函数栈上 4 KiB 可写
    // 缓冲，buffersize 即其长度（proc_pidpath 按上限截断写入，契约保证不
    // 越界）；raw_pid 已受检；调用期间缓冲存活且不被别名引用。
    let written = unsafe {
        proc_pidpath(
            raw_pid,
            buffer.as_mut_ptr().cast::<libc::c_void>(),
            PROC_PIDPATHINFO_MAXSIZE,
        )
    };
    if written <= 0 {
        return Ok(None);
    }
    // 扫描上限：已写入部分 + 其后的一个字节（proc_pidpath 契约保证 NUL
    // 终止；返回长度不含 NUL 时，紧随其后的零填充字节即 NUL，二者皆被覆盖）。
    let written = usize::try_from(written).unwrap_or(0).min(buffer.len());
    let scan_end = written.saturating_add(1).min(buffer.len());
    let scanned = buffer.get(..scan_end).unwrap_or(&[]);
    // 结构性 NUL 保证：缓冲内无 NUL 的异常情形（契约违例）返回类型化失败，
    // 不经指针重建 CStr、不 panic。
    let path = std::ffi::CStr::from_bytes_until_nul(scanned).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("pid {pid} 的 proc_pidpath 未返回 NUL 终止路径"),
        )
    })?;
    let text = String::from_utf8_lossy(path.to_bytes()).into_owned();
    Ok((!text.is_empty()).then_some(text))
}

/// 任务的内存与线程信息（witr `readDarwinTaskInfo` 语义：ESRCH/EPERM 视为
/// 无数据，其余 errno 为错误）。`Ok(None)` = 进程已退出或权限不足。
pub(crate) fn task_snapshot(pid: u32) -> io::Result<Option<TaskSnapshot>> {
    let raw_pid = to_c_int(pid);
    let mut info = ProcTaskInfo {
        pti_virtual_size: 0,
        pti_resident_size: 0,
        pti_total_user: 0,
        pti_total_system: 0,
        pti_threads_user: 0,
        pti_threads_system: 0,
        pti_policy: 0,
        pti_faults: 0,
        pti_pageins: 0,
        pti_cow_faults: 0,
        pti_messages_sent: 0,
        pti_messages_received: 0,
        pti_syscalls_mach: 0,
        pti_syscalls_unix: 0,
        pti_csw: 0,
        pti_threadnum: 0,
        pti_numrunning: 0,
        pti_priority: 0,
    };
    // SAFETY: [Category 8 - FFI boundary]。info 为 repr(C)，字段与 SDK
    // `proc_taskinfo` 全量对齐（96 字节编译期断言）；指针指向栈上独占缓冲，
    // 调用期间无别名；返回值 < 0 表示失败，(0, 96) 表示不完整写入。
    let written = unsafe {
        proc_pidinfo(
            raw_pid,
            PROC_PIDTASKINFO,
            0,
            (&mut info as *mut ProcTaskInfo).cast::<libc::c_void>(),
            PROC_PIDTASKINFO_SIZE,
        )
    };
    if written < 0 {
        return no_data_or_error(pid, "proc_pidinfo");
    }
    if written < PROC_PIDTASKINFO_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("pid {pid} 的 proc_pidinfo 返回 {written} 字节，不足 {PROC_PIDTASKINFO_SIZE}"),
        ));
    }
    Ok(Some(TaskSnapshot {
        virtual_bytes: info.pti_virtual_size,
        resident_bytes: info.pti_resident_size,
        threads: info.pti_threadnum,
    }))
}

/// 进程 FD 总数（witr `readDarwinFDs`：`PROC_PIDLISTFDS`；缓冲不足（EINVAL）
/// 时按 witr 语义翻倍重试至 16384 条上限）。
pub(crate) fn fd_count(pid: u32) -> io::Result<Option<u64>> {
    let raw_pid = to_c_int(pid);
    // `struct proc_fdinfo { int proc_fd; uint32_t proc_fdtype; }` = 8 字节。
    const ENTRY_SIZE: usize = 8;
    let mut entries: usize = 256;
    loop {
        let mut buffer = vec![0u8; entries * ENTRY_SIZE];
        // SAFETY: [Category 8 - FFI boundary]。buffer 为堆上独占可写缓冲，
        // 长度按 8 字节/条换算并逐次翻倍；内核按缓冲上限截断，写入量由返回
        // 值（字节数）表达。
        let written = unsafe {
            proc_pidinfo(
                raw_pid,
                PROC_PIDLISTFDS,
                0,
                buffer.as_mut_ptr().cast::<libc::c_void>(),
                libc::c_int::try_from(buffer.len()).unwrap_or(libc::c_int::MAX),
            )
        };
        if written < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EINVAL) && entries < 16_384 {
                entries *= 2;
                continue;
            }
            if error.raw_os_error() == Some(libc::EINVAL) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("pid {pid} 的 FD 表超过缓冲上限（16384 条）"),
                ));
            }
            return no_data_or_error(pid, "proc_pidinfo");
        }
        let used = usize::try_from(written).unwrap_or(0);
        if used % ENTRY_SIZE != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("pid {pid} 的 PROC_PIDLISTFDS 返回部分记录（{used} 字节）"),
            ));
        }
        return Ok(Some(u64::try_from(used / ENTRY_SIZE).unwrap_or(u64::MAX)));
    }
}

/// 磁盘 I/O 字节统计（witr `readDarwinIO` 语义：ESRCH/EPERM 视为无数据）。
pub(crate) fn disk_io(pid: u32) -> io::Result<Option<IoStats>> {
    let raw_pid = to_c_int(pid);
    // 1 KiB、8 字节对齐的零填充缓冲：即使 SDK 的 rusage_info_v4 大于本模块
    // 声明的偏移布局，内核写入也不会越界。
    let mut buffer = [0u64; 128];
    // SAFETY: [Category 8 - FFI boundary]。buffer 为栈上独占、8 字节对齐、
    // 1024 字节（≥ SDK rusage_info_v4 全量布局的保守上界）；指针在调用期间
    // 不被别名；flavor 为值类型常量 RUSAGE_INFO_V4。
    let result = unsafe {
        proc_pid_rusage(
            raw_pid,
            RUSAGE_INFO_V4,
            buffer.as_mut_ptr().cast::<libc::c_void>(),
        )
    };
    if result != 0 {
        return no_data_or_error(pid, "proc_pid_rusage");
    }
    Ok(Some(IoStats {
        read_bytes: read_u64_at(&buffer, RUSAGE_DISKIO_BYTESREAD_OFFSET),
        write_bytes: read_u64_at(&buffer, RUSAGE_DISKIO_BYTESWRITTEN_OFFSET),
        read_ops: 0,
        write_ops: 0,
    }))
}

/// libproc 失败分类（witr 同语义）：ESRCH / EPERM 视为「无数据」，其余为
/// 类型化错误。
fn no_data_or_error<T>(pid: u32, operation: &str) -> io::Result<Option<T>> {
    let error = io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::ESRCH) | Some(libc::EPERM) => Ok(None),
        _ => Err(io::Error::other(format!(
            "pid {pid} 的 {operation} 失败：{error}"
        ))),
    }
}

/// 从 8 字节对齐缓冲按字节偏移读取 u64（macOS AA64 与 Intel 均为小端；
/// 偏移由 8 整除）。
fn read_u64_at(buffer: &[u64], byte_offset: usize) -> u64 {
    buffer.get(byte_offset / 8).copied().unwrap_or_default()
}

fn to_c_int(pid: u32) -> libc::c_int {
    // 平台 PID 表示范围（pid_t）内的受检转换；溢出值钳制为 0，后续内核
    // 调用按 ESRCH 落入「无数据」路径。
    libc::c_int::try_from(pid).unwrap_or(0)
}