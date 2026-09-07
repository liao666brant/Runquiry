//! SCM（Service Control Manager）FFI 安全包装（仅 `target_os = "windows"` 编译）。
//!
//! 与 `ffi` 同风格：句柄经 RAII 守卫在析构时 `CloseServiceHandle`，结构体
//! 输出缓冲区以 8 字节对齐分配（`ENUM_SERVICE_STATUS_PROCESSW` /
//! `QUERY_SERVICE_CONFIGW` / `SERVICE_DESCRIPTIONW` 均含指针成员），解析交
//! 由纯逻辑模块 `scm_parse` 在缓冲区存活期内完成。两阶段 sizing：先以 NULL
//! 缓冲区探询所需字节数，再按该尺寸分配读取，缓冲区总量有上限。每个
//! `unsafe` 块的 SAFETY 注释说明指针、长度、对齐、生命周期与所有权。所需
//! 函数均由 windows-sys 的 `Win32_System_Services` feature 导出，本文件不
//! 包含手写 extern；Linux 不编译本文件，纯解析行为由 `tests/windows_scm.rs`
//! 经 `#[path]` 直接验证。

use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::System::Services::{
    QUERY_SERVICE_CONFIGW, SC_ENUM_PROCESS_INFO, SC_HANDLE, SC_MANAGER_ENUMERATE_SERVICE,
    SERVICE_CONFIG_DESCRIPTION, SERVICE_QUERY_CONFIG, SERVICE_STATE_ALL, SERVICE_WIN32,
    CloseServiceHandle, EnumServicesStatusExW, OpenSCManagerW, OpenServiceW,
    QueryServiceConfig2W, QueryServiceConfigW,
};

use super::scm_parse::{ServiceConfig, parse_query_service_config, parse_service_description};
use super::winerror::{ERROR_INSUFFICIENT_BUFFER, ERROR_MORE_DATA, Win32Error};

/// SCM 枚举缓冲区防御上限（4 MiB：真实服务表远小于此，超出视为系统异常）。
const MAX_ENUM_BYTES: usize = 4 * 1024 * 1024;

/// SCM 采集失败（Win32 错误码或防御上限触发；文案由调用方组装）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScmError {
    /// Win32 调用失败（`GetLastError` 原值）。
    Win32(u32),
    /// 枚举缓冲区超出防御上限。
    Oversize {
        /// 系统报告的所需字节数。
        requested: u32,
        /// 允许的最大字节数。
        cap: u32,
    },
}

impl From<Win32Error> for ScmError {
    fn from(error: Win32Error) -> Self {
        Self::Win32(error.0)
    }
}

/// 超限错误（`cap` 恒为防御上限的 u32 视图）。
fn oversize_error(requested: u32) -> ScmError {
    ScmError::Oversize { requested, cap: u32::try_from(MAX_ENUM_BYTES).unwrap_or(u32::MAX) }
}

/// 本机 SCM 数据库句柄的 RAII 守卫（析构时 `CloseServiceHandle`）。
struct ScManagerGuard(SC_HANDLE);

impl ScManagerGuard {
    /// 以只读枚举权限打开本机 SCM 数据库。
    ///
    /// # Errors
    /// 返回 NULL 句柄（SCM 不可用 / 权限不足）时返回 `GetLastError`。
    fn open_readonly() -> Result<Self, Win32Error> {
        // SAFETY：机器名与数据库名均传 NULL（API 契约：本机
        // SERVICES_ACTIVE_DATABASE）；返回 NULL 表示失败，错误码经
        // GetLastError 取得。句柄所有权立即移入守卫，析构时
        // CloseServiceHandle，不复制、不提前关闭。
        let handle = unsafe {
            OpenSCManagerW(core::ptr::null(), core::ptr::null(), SC_MANAGER_ENUMERATE_SERVICE)
        };
        if handle.is_null() {
            return Err(Win32Error(unsafe { GetLastError() }));
        }
        Ok(Self(handle))
    }

    /// 原始句柄（只读借用；调用方不得关闭或复制所有权）。
    const fn raw(&self) -> SC_HANDLE {
        self.0
    }
}

impl Drop for ScManagerGuard {
    fn drop(&mut self) {
        // SAFETY：self.0 来自 OpenSCManagerW 成功返回且未被复制或提前关闭；
        // CloseServiceHandle 兼容 SC_MANAGER 与服务两类句柄；Drop 恰好执行一次。
        unsafe {
            CloseServiceHandle(self.0);
        }
    }
}

/// 单个服务句柄的 RAII 守卫（析构时 `CloseServiceHandle`）。
struct ServiceGuard(SC_HANDLE);

impl ServiceGuard {
    /// 以 `SERVICE_QUERY_CONFIG` 打开服务（名称为 NUL 终止 UTF-16）。
    ///
    /// # Errors
    /// 返回 NULL 句柄（服务不存在 / 权限不足 / 名称非法）时返回 `GetLastError`。
    fn open_query_config(manager: &ScManagerGuard, wide_name: &[u16]) -> Result<Self, Win32Error> {
        // SAFETY：manager.raw() 为有效的 SCM 句柄；wide_name 以 NUL 终止且
        // 生命周期覆盖本次调用；返回 NULL 表示失败。句柄所有权立即移入守卫。
        let handle =
            unsafe { OpenServiceW(manager.raw(), wide_name.as_ptr(), SERVICE_QUERY_CONFIG) };
        if handle.is_null() {
            return Err(Win32Error(unsafe { GetLastError() }));
        }
        Ok(Self(handle))
    }

    /// 原始句柄（只读借用）。
    const fn raw(&self) -> SC_HANDLE {
        self.0
    }
}

impl Drop for ServiceGuard {
    fn drop(&mut self) {
        // SAFETY：self.0 来自 OpenServiceW 成功返回且未被复制或提前关闭；
        // Drop 恰好执行一次。
        unsafe {
            CloseServiceHandle(self.0);
        }
    }
}

/// 服务名 → NUL 终止 UTF-16（嵌入 NUL 的名称非法，返回 `None` 由调用方跳过）。
fn wide_service_name(name: &str) -> Option<Vec<u16>> {
    if name.contains('\0') {
        return None;
    }
    let mut units: Vec<u16> = name.encode_utf16().chain(core::iter::once(0)).collect();
    Some(units)
}

/// SCM 枚举缓冲区：8 字节对齐分配，条目内的字符串指针指向缓冲区内地址。
pub struct EnumBuffer {
    /// 8 字节对齐分配（`Vec<u64>` 承载；有效字节数由系统填充量决定）。
    aligned: Vec<u64>,
    /// 系统报告的条目数。
    count: u32,
}

impl EnumBuffer {
    /// 空枚举（无匹配服务 / 系统未报告尺寸；无数据可解析）。
    const fn empty() -> Self {
        Self {
            aligned: Vec::new(),
            count: 0,
        }
    }

    /// 全缓冲区的字节视图（解析时的 `base` 必须取
    /// `self.bytes().as_ptr() as usize`，与字符串指针的绝对地址一致）。
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        let byte_len = self.aligned.len() * size_of::<u64>();
        // SAFETY：aligned 为本结构持有的已初始化分配（零值起步 + FFI 写入）；
        // 以 u8 视图重建切片长度不超过分配大小，且读取一律按字节拷贝
        //（scm_parse），不通过 u64 引用产生别名。
        unsafe { core::slice::from_raw_parts(self.aligned.as_ptr().cast::<u8>(), byte_len) }
    }

    /// 系统报告的条目数。
    #[must_use]
    pub const fn count(&self) -> u32 {
        self.count
    }
}

/// 按字节数分配 8 字节对齐缓冲区（超出防御上限返回 `None`）。
fn aligned_buffer(byte_len: usize) -> Option<Vec<u64>> {
    if byte_len > MAX_ENUM_BYTES {
        return None;
    }
    Some(vec![0u64; byte_len.div_ceil(size_of::<u64>())])
}

/// 全量枚举 SCM 服务（`SERVICE_WIN32 | SERVICE_STATE_ALL`，两阶段 sizing）。
///
/// # Errors
/// SCM 不可用 / 枚举失败 / 缓冲区超限 / 两次读取间条目持续增长时返回
/// [`ScmError`]；成功返回的缓冲区交 `scm_parse::parse_enum_buffer` 解析
/// （`base` 取 `bytes().as_ptr()`）。
#[must_use]
pub fn enumerate_services() -> Result<EnumBuffer, ScmError> {
    let manager = ScManagerGuard::open_readonly()?;
    let mut needed: u32 = 0;
    let mut returned: u32 = 0;
    let mut resume: u32 = 0;
    // SAFETY：第一阶段以 NULL 缓冲区探询尺寸（API 契约：cbBufSize=0 时把
    // 所需字节数写入 pcbBytesNeeded 并返回 ERROR_MORE_DATA /
    // ERROR_INSUFFICIENT_BUFFER）；三个出参均为栈上 u32，生命周期覆盖调用。
    let probe = unsafe {
        EnumServicesStatusExW(
            manager.raw(),
            SC_ENUM_PROCESS_INFO,
            SERVICE_WIN32,
            SERVICE_STATE_ALL,
            core::ptr::null_mut(),
            0,
            &mut needed,
            &mut returned,
            &mut resume,
            core::ptr::null(),
        )
    };
    if probe != 0 {
        return Ok(EnumBuffer::empty());
    }
    let probe_error = Win32Error(unsafe { GetLastError() });
    if probe_error.0 != ERROR_MORE_DATA && probe_error.0 != ERROR_INSUFFICIENT_BUFFER {
        return Err(probe_error.into());
    }
    if needed == 0 {
        // 系统未报告尺寸：按空枚举处理，不伪造数据。
        return Ok(EnumBuffer::empty());
    }
    if u32::try_from(MAX_ENUM_BYTES).is_ok_and(|cap| needed > cap) {
        return Err(oversize_error(needed));
    }
    // 两次调用间条目可能增长：按系统报告的新尺寸重试，最多三阶段。
    let mut requested = needed;
    let mut attempts = 0;
    while attempts < 3 {
        attempts += 1;
        let Some(mut buffer) = aligned_buffer(requested as usize) else {
            return Err(oversize_error(requested));
        };
        let capacity = u32::try_from(buffer.len() * size_of::<u64>()).unwrap_or(0);
        // SAFETY：buffer 为 8 字节对齐分配（ENUM_SERVICE_STATUS_PROCESSW 含
        // 指针成员，要求指针宽度对齐），capacity 以字节计且与 cbBufSize
        // 契约一致；lpResumeHandle 承接系统游标（首次为 0，重试续读）；返回
        // 0 视为失败，ERROR_MORE_DATA 表示条目增长、按新尺寸重试。
        let ok = unsafe {
            EnumServicesStatusExW(
                manager.raw(),
                SC_ENUM_PROCESS_INFO,
                SERVICE_WIN32,
                SERVICE_STATE_ALL,
                buffer.as_mut_ptr().cast::<u8>(),
                capacity,
                &mut needed,
                &mut returned,
                &mut resume,
                core::ptr::null(),
            )
        };
        if ok != 0 {
            return Ok(EnumBuffer {
                aligned: buffer,
                count: returned,
            });
        }
        let error = Win32Error(unsafe { GetLastError() });
        if (error.0 == ERROR_MORE_DATA || error.0 == ERROR_INSUFFICIENT_BUFFER)
            && needed > requested
        {
            requested = needed;
            continue;
        }
        return Err(error.into());
    }
    Err(ScmError::Win32(ERROR_MORE_DATA))
}

/// 打开服务句柄并执行一次配置查询（打开失败 → `None`，调用方省略对应键）。
fn with_query_target<T>(name: &str, run: impl FnOnce(SC_HANDLE) -> Option<T>) -> Option<T> {
    let manager = ScManagerGuard::open_readonly().ok()?;
    let wide_name = wide_service_name(name)?;
    let service = ServiceGuard::open_query_config(&manager, &wide_name).ok()?;
    run(service.raw())
}

/// 两阶段查询骨架：NULL 探询尺寸 → 8 字节对齐分配 → 读取 → 解析（缓冲区
/// 存活期内完成解析；任一阶段失败返回 `None`，不伪造值）。
fn query_two_stage<T>(
    service: SC_HANDLE,
    probe: impl Fn(SC_HANDLE, *mut u32) -> i32,
    read: impl Fn(SC_HANDLE, *mut u8, u32, *mut u32) -> i32,
    parse: impl FnOnce(&[u8], usize) -> Option<T>,
) -> Option<T> {
    let mut needed: u32 = 0;
    // SAFETY：探询尺寸：NULL 缓冲区 + cbBufSize=0，API 把所需字节数写入
    // pcbBytesNeeded；needed 为栈上 u32 出参，生命周期覆盖调用。
    if probe(service, &mut needed) == 0 {
        let error = Win32Error(unsafe { GetLastError() });
        if error.0 != ERROR_INSUFFICIENT_BUFFER && error.0 != ERROR_MORE_DATA {
            return None;
        }
    }
    if needed == 0 {
        return None;
    }
    let mut buffer = aligned_buffer(needed as usize)?;
    let capacity = u32::try_from(buffer.len() * size_of::<u64>()).ok()?;
    // SAFETY：buffer 为 8 字节对齐分配（输出结构均含指针成员，要求指针
    // 宽度对齐），capacity 以字节计与 cbBufSize 契约一致；成功时系统把
    // 结构与字符串写入缓冲区；needed 复用为实际所需字节数。
    if read(service, buffer.as_mut_ptr().cast::<u8>(), capacity, &mut needed) == 0 {
        return None;
    }
    parse(valid_bytes(&buffer, needed), buffer.as_ptr() as usize)
}

/// 单服务的启动配置（服务不存在 / 权限不足 / 查询或解析失败 → `None`）。
#[must_use]
pub fn query_service_config(name: &str) -> Option<ServiceConfig> {
    with_query_target(name, |service| {
        query_two_stage(
            service,
            // SAFETY：QueryServiceConfigW 以 NULL 缓冲区探询所需字节数；
            // needed 为栈上 u32 出参。
            |handle, needed| unsafe {
                QueryServiceConfigW(handle, core::ptr::null_mut(), 0, needed)
            },
            // SAFETY：buffer 为 8 字节对齐分配（QUERY_SERVICE_CONFIGW 含
            // 指针成员），capacity 以字节计与 cbBufSize 契约一致。
            |handle, buffer, capacity, needed| unsafe {
                QueryServiceConfigW(
                    handle,
                    buffer.cast::<QUERY_SERVICE_CONFIGW>(),
                    capacity,
                    needed,
                )
            },
            |bytes, base| parse_query_service_config(bytes, base).ok(),
        )
    })
}

/// 单服务的描述文本（服务不存在 / 无描述 / 失败 → `None`）。
#[must_use]
pub fn query_service_description(name: &str) -> Option<String> {
    with_query_target(name, |service| {
        query_two_stage(
            service,
            // SAFETY：QueryServiceConfig2W 以 NULL 缓冲区探询
            // SERVICE_CONFIG_DESCRIPTION 所需字节数；无描述的服务可能直接
            // 成功且 needed=0（按无描述处理）；needed 为栈上 u32 出参。
            |handle, needed| unsafe {
                QueryServiceConfig2W(
                    handle,
                    SERVICE_CONFIG_DESCRIPTION,
                    core::ptr::null_mut(),
                    0,
                    needed,
                )
            },
            // SAFETY：buffer 为 8 字节对齐分配（SERVICE_DESCRIPTIONW 含
            // 指针成员），capacity 以字节计与 cbBufSize 契约一致。
            |handle, buffer, capacity, needed| unsafe {
                QueryServiceConfig2W(handle, SERVICE_CONFIG_DESCRIPTION, buffer, capacity, needed)
            },
            |bytes, base| parse_service_description(bytes, base).ok(),
        )
    })
}

/// 成功调用后的有效字节视图（系统报告的所需字节数与缓冲区容量取小；
/// 解析按该边界校验指针，绝不越出分配）。
fn valid_bytes(buffer: &[u64], needed: u32) -> &[u8] {
    let valid = usize::try_from(needed).unwrap_or(0).min(buffer.len() * size_of::<u64>());
    // SAFETY：以 u8 视图读取 8 字节对齐分配的前 `valid` 字节；长度不超过
    // 分配容量，且该内存已由 FFI 初始化（失败路径不会到达此处）。
    unsafe { core::slice::from_raw_parts(buffer.as_ptr().cast::<u8>(), valid) }
}