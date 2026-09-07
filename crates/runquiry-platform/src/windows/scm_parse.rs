//! SCM（Service Control Manager）缓冲区与配置的纯解析（无 OS 依赖）。
//!
//! 本文件依赖 `std` + `runquiry-core`，可在任何平台编译；Linux 上经
//! `#[path = "../src/windows/scm_parse.rs"]` 由 `tests/windows_scm.rs` 直接
//! 执行（配合同模块树的 `utf16`）。布局与 `windows-sys 0.61.2` 的
//! `ENUM_SERVICE_STATUS_PROCESSW` / `QUERY_SERVICE_CONFIGW` /
//! `SERVICE_DESCRIPTIONW` ABI 一致（x64 与 x86 皆可，指针宽度取
//! `size_of::<usize>()`）。字符串字段是指向同缓冲区尾部的绝对指针，
//! 解析需调用方提供缓冲区基址（FFI 传 `as_ptr() as usize`，纯测试传
//! 合成基址）。语义对齐 witr `services_windows.go`：PID 0（服务已停止）
//! 跳过、空服务名跳过、共享宿主（svchost）按 PID 首写者胜。

use super::utf16::{self, NUL_STRING_MAX_UNITS, Utf16Error};

/// 服务条目（枚举缓冲区中的一行；`pid_raw == 0` 表示服务未运行）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RawServiceEntry {
    /// SCM 服务名（写入 core 证据的 `service` 键）。
    pub name: String,
    /// 显示名（`display_name` 键）。
    pub display_name: String,
    /// `dwCurrentState` 原值。
    pub state_raw: u32,
    /// `dwProcessId` 原值（0 = 服务已停止）。
    pub pid_raw: u32,
}

/// SCM 缓冲区解析错误（文案稳定，不携带缓冲区内容）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScmParseError {
    /// 缓冲区不足以容纳 `count` 个条目（枚举被截断或计数损坏）。
    BufferTooShort {
        /// 声明的条目数。
        count: u32,
        /// 按条目数推算的所需字节数。
        needed: usize,
        /// 实际缓冲区字节数。
        actual: usize,
    },
    /// 条目内的字符串指针指向缓冲区之外（计数与缓冲区不匹配）。
    PointerOutOfRange,
    /// 字符串在扫描上限内无 NUL 终止符（缓冲区损坏）。
    NoTerminator,
}

impl From<Utf16Error> for ScmParseError {
    fn from(_: Utf16Error) -> Self {
        Self::NoTerminator
    }
}

/// 单个条目的字节跨度：名称 / 显示名指针 + 9 × u32 的 SERVICE_STATUS_PROCESS，
/// 按指针宽度对齐。
#[must_use]
pub(super) fn enum_entry_stride() -> usize {
    let ptr = size_of::<usize>();
    let status_bytes = 9 * 4;
    let unaligned = ptr * 2 + status_bytes;
    unaligned.div_ceil(ptr) * ptr
}

/// 解析 `EnumServicesStatusExW` 输出缓冲区为服务条目（不跳过 PID 0，
/// 由调用方按 witr 规则跳过；条目按缓冲区顺序返回）。
///
/// # Errors
/// 缓冲区与计数不一致 / 指针越界 / 字符串无终止符时返回 [`ScmParseError`]。
pub(super) fn parse_enum_buffer(
    buf: &[u8],
    base: usize,
    count: u32,
) -> Result<Vec<RawServiceEntry>, ScmParseError> {
    let stride = enum_entry_stride();
    let needed = count as usize * stride;
    if buf.len() < needed {
        return Err(ScmParseError::BufferTooShort {
            count,
            needed,
            actual: buf.len(),
        });
    }
    let mut entries = Vec::with_capacity(count as usize);
    for index in 0..count as usize {
        let base_offset = index * stride;
        // SERVICE_STATUS_PROCESS 起始偏移 = 2 × 指针宽度；状态 @ +4，PID @ +28。
        let status_offset = base_offset + size_of::<usize>() * 2;
        let state_raw =
            read_u32_le(buf, status_offset + 4).ok_or(ScmParseError::BufferTooShort {
                count,
                needed,
                actual: buf.len(),
            })?;
        let pid_raw =
            read_u32_le(buf, status_offset + 28).ok_or(ScmParseError::BufferTooShort {
                count,
                needed,
                actual: buf.len(),
            })?;
        let name = read_pointer_string(buf, base, base_offset)?;
        let display_name = read_pointer_string(buf, base, base_offset + size_of::<usize>())?;
        entries.push(RawServiceEntry {
            name,
            display_name,
            state_raw,
            pid_raw,
        });
    }
    Ok(entries)
}

/// 按 PID 首写者胜出去重（witr `serviceMapForPIDs`：共享宿主 svchost.exe
/// 保持跨调用稳定的服务名；PID 0 与空服务名跳过）。
#[must_use]
pub(super) fn dedup_by_pid(entries: Vec<RawServiceEntry>) -> Vec<(u32, RawServiceEntry)> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for entry in entries {
        if entry.pid_raw == 0 {
            // 服务注册但未运行：无宿主 PID，跳过（witr 同规则）。
            continue;
        }
        if entry.name.is_empty() {
            continue;
        }
        if seen.insert(entry.pid_raw) {
            result.push((entry.pid_raw, entry));
        }
    }
    result
}

/// `dwCurrentState` → 状态名（PowerShell `Get-Service` 风格，与 SCM 常量对应）。
#[must_use]
pub(super) const fn service_state_name(state_raw: u32) -> &'static str {
    match state_raw {
        1 => "Stopped",
        2 => "Start Pending",
        3 => "Stop Pending",
        4 => "Running",
        5 => "Continue Pending",
        6 => "Pause Pending",
        7 => "Paused",
        _ => "Unknown",
    }
}

/// `QueryServiceConfigW` 的解析结果（账户字段不采集：core 键契约不含账户，
/// 敏感账户名不进入证据）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ServiceConfig {
    /// `dwStartType` 原值。
    pub start_raw: u32,
    /// 二进制路径（`binary_path` 键）。
    pub binary_path: Option<String>,
}

/// 解析 `QueryServiceConfigW` 输出缓冲区（x64：type@0, start@4,
/// binary@16；指针指向缓冲区尾部）。
///
/// # Errors
/// 缓冲区过短 / 指针越界 / 字符串无终止符时返回 [`ScmParseError`]。
pub(super) fn parse_query_service_config(
    buf: &[u8],
    base: usize,
) -> Result<ServiceConfig, ScmParseError> {
    let start_raw = read_u32_le(buf, 4).ok_or(ScmParseError::PointerOutOfRange)?;
    let binary_path = pointer_string_at(buf, base, offset_of_binary_path());
    Ok(ServiceConfig {
        start_raw,
        binary_path,
    })
}

/// `lpBinaryPathName` 在 `QUERY_SERVICE_CONFIGW` 内的字节偏移。
#[must_use]
pub(super) const fn offset_of_binary_path() -> usize {
    // type(4) + start(4) + error(4) + 对齐填充(4) → 第一个指针位于 16。
    16
}

/// 解析 `QueryServiceConfig2W(SERVICE_CONFIG_DESCRIPTION)` 输出缓冲区
/// （`SERVICE_DESCRIPTIONW { lpDescription }`，指针 @0）。
///
/// # Errors
/// 指针越界 / 字符串无终止符时返回 [`ScmParseError`]。
pub(super) fn parse_service_description(buf: &[u8], base: usize) -> Result<String, ScmParseError> {
    pointer_string_at(buf, base, 0).ok_or(ScmParseError::PointerOutOfRange)
}

/// 读取缓冲区内的 LE u32；越界返回 `None`。
fn read_u32_le(buf: &[u8], offset: usize) -> Option<u32> {
    if offset + 4 > buf.len() {
        return None;
    }
    Some(u32::from_le_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ]))
}

/// 读取缓冲区内的 LE 指针（宽度随平台；32 位零扩展）。
fn read_pointer(buf: &[u8], offset: usize) -> Option<usize> {
    let width = size_of::<usize>();
    if offset + width > buf.len() {
        return None;
    }
    let mut value = 0usize;
    for (index, byte) in buf[offset..offset + width].iter().enumerate() {
        value |= usize::from(*byte) << (index * 8);
    }
    Some(value)
}

/// 把缓冲区内指针字段解析为字符串（指针绝对地址 → 相对偏移）。
fn read_pointer_string(buf: &[u8], base: usize, offset: usize) -> Result<String, ScmParseError> {
    pointer_string_at(buf, base, offset).ok_or(ScmParseError::PointerOutOfRange)
}

/// 偏移处的指针 → 缓冲区内 NUL 终止 UTF-16 字符串；越界返回 `None`。
fn pointer_string_at(buf: &[u8], base: usize, offset: usize) -> Option<String> {
    let pointer = read_pointer(buf, offset)?;
    if pointer < base || pointer > base + buf.len() {
        return None;
    }
    let start = pointer - base;
    let tail = buf.get(start..)?;
    let units: Vec<u16> = tail
        .chunks_exact(2)
        .take(NUL_STRING_MAX_UNITS)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    utf16::nul_terminated_bounded(&units, NUL_STRING_MAX_UNITS).ok()
}
