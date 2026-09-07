//! UNICODE_STRING 校验、ProcessParameters 字段提取与环境块上限（纯逻辑）。

use super::super::utf16::{ENV_CHUNK_BYTES, MAX_ENV_BLOCK_BYTES, MAX_STRING_BYTES};
use super::layout::{PebLayout, extract_pointer};

/// 已校验的远程字符串引用：`buffer` 非零、`length_bytes` 有界且不超过
/// `MaximumLength`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteString {
    /// 远端 Buffer 地址。
    pub buffer: u64,
    /// 字节长度（偶数化前的原始值，读取时截断到完整单元）。
    pub length_bytes: u32,
}

/// UNICODE_STRING 校验失败（文案稳定；`field` 为字段名，不携带数据）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnicodeStringError {
    /// `Length` 为 0 或 Buffer 为空。
    Empty,
    /// `Length` 超过 `MaximumLength`（读取中 PEB 被改写的典型痕迹）。
    LengthExceedsMaximum,
    /// 超出防御上限（32 KiB）。
    Oversize,
}

/// 校验 UNICODE_STRING 的 Length/MaximumLength 并折算为字节长度。
///
/// 规则（witr `readUnicodeString` + 契约）：字节长非空、不超过
/// [`MAX_STRING_BYTES`]、不超过 `MaximumLength`；奇数字节长保留原值
/// （解码时丢弃尾字节，与 witr 的整单元截断一致）。
///
/// # Errors
/// 违反任一规则时返回 [`UnicodeStringError`]。
pub const fn validate_unicode_string(
    length_bytes: u16,
    max_length: u16,
) -> Result<u16, UnicodeStringError> {
    if length_bytes == 0 {
        return Err(UnicodeStringError::Empty);
    }
    if (length_bytes as u32) > MAX_STRING_BYTES {
        return Err(UnicodeStringError::Oversize);
    }
    if length_bytes > max_length {
        return Err(UnicodeStringError::LengthExceedsMaximum);
    }
    Ok(length_bytes)
}

/// 从已读 ProcessParameters 缓冲区提取一个远程字符串（cwd / image / cmdline）。
///
/// 返回 `None` 表示该字段不可得（偏移越界、空串、校验失败），调用方按
/// 字段记诊断；其余字段不受影响（部分结果语义）。
///
/// # Errors
/// 缓冲区不足以覆盖该字段时返回 [`PebFieldError::ParamsTooShort`]。
pub fn remote_string_field(
    params: &[u8],
    layout: &PebLayout,
    field_offset: usize,
) -> Result<Option<RemoteString>, PebFieldError> {
    let field_end = field_offset + layout.unicode_string_bytes;
    if params.len() < field_end {
        return Err(PebFieldError::ParamsTooShort {
            needed: field_end,
            actual: params.len(),
        });
    }
    let us = &params[field_offset..field_end];
    let length_bytes = u16::from_le_bytes([us[0], us[1]]);
    let max_length = u16::from_le_bytes([us[2], us[3]]);
    let buffer = extract_pointer(
        &us[layout.string_buffer_offset..layout.unicode_string_bytes],
        layout.pointer_bytes,
    )
    .ok_or(PebFieldError::PointerExtract)?;
    if buffer == 0 {
        return Ok(None);
    }
    let length_bytes = validate_unicode_string(length_bytes, max_length)
        .map_err(|_| PebFieldError::InvalidUnicodeString)?;
    Ok(Some(RemoteString {
        buffer,
        length_bytes: u32::from(length_bytes),
    }))
}

/// 单字段提取失败（文案由调用方组装）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PebFieldError {
    /// ProcessParameters 缓冲区不足以覆盖该字段（读取中进程退出 / PEB 变化）。
    ParamsTooShort {
        /// 需要的字节数。
        needed: usize,
        /// 实际读取字节数。
        actual: usize,
    },
    /// Buffer 指针提取失败（宽度与布局不符）。
    PointerExtract,
    /// Length/MaximumLength 校验失败。
    InvalidUnicodeString,
}

/// 从已读 ProcessParameters 缓冲区提取 Environment 块指针；缺省为 0。
#[must_use]
pub fn environment_pointer(params: &[u8], layout: &PebLayout) -> Option<u64> {
    let end = layout.env_offset + layout.pointer_bytes;
    if params.len() < end {
        return None;
    }
    extract_pointer(&params[layout.env_offset..end], layout.pointer_bytes)
}

/// 环境块读取上限（暴露给读取器：总量与单块字节数来自 utf16 常量）。
#[must_use]
pub const fn env_block_limits() -> (usize, usize) {
    (MAX_ENV_BLOCK_BYTES, ENV_CHUNK_BYTES)
}

#[cfg(test)]
mod tests {
    // `ENV_CHUNK_BYTES` 等来自 fields 顶部的 `use super::super::utf16`
    // 私有导入（子模块可见），避免三重 `super::` 在 `#[path]` 测试上下文越界。
    use super::{
        ENV_CHUNK_BYTES, MAX_ENV_BLOCK_BYTES, MAX_STRING_BYTES, PebFieldError, PebLayout,
        RemoteString, UnicodeStringError, environment_pointer, env_block_limits,
        remote_string_field, validate_unicode_string,
    };

    fn us64(length: u16, max: u16, buffer: u64) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&length.to_le_bytes());
        bytes.extend_from_slice(&max.to_le_bytes());
        bytes.extend_from_slice(&[0, 0, 0, 0]); // 对齐填充
        bytes.extend_from_slice(&buffer.to_le_bytes());
        bytes
    }

    /// 单字段提取的取值助手：`PebFieldError` 未实现 `std::error::Error`
    /// （不为此改动生产公共 API），Result/None 均映射为可传播的 String。
    fn remote_of(
        params: &[u8],
        layout: &PebLayout,
        offset: usize,
    ) -> Result<RemoteString, String> {
        match remote_string_field(params, layout, offset) {
            Ok(Some(remote)) => Ok(remote),
            Ok(None) => Err(String::from("合成数据应产生非空字段")),
            Err(error) => Err(format!("字段提取失败：{error:?}")),
        }
    }

    #[test]
    fn remote_string_field_validates_64_bit_unicode_string(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let layout = PebLayout::win64();
        let mut params = vec![0u8; 136];
        let us = us64(26, 32, 0x0000_7FF6_ABCD_0000);
        params[0x70..0x80].copy_from_slice(&us);
        let field = remote_of(&params, &layout, layout.cmdline_offset)?;
        assert_eq!(
            field,
            RemoteString {
                buffer: 0x0000_7FF6_ABCD_0000,
                length_bytes: 26,
            }
        );
        Ok(())
    }

    #[test]
    fn remote_string_field_rejects_oversize_and_mismatch() {
        let layout = PebLayout::win64();
        let mut params = vec![0u8; 136];
        // 超上限。
        params[0x70..0x80].copy_from_slice(&us64(u16::MAX, u16::MAX, 0x10));
        assert_eq!(
            remote_string_field(&params, &layout, layout.cmdline_offset),
            Err(PebFieldError::InvalidUnicodeString)
        );
        // Length > MaximumLength。
        params[0x70..0x80].copy_from_slice(&us64(100, 50, 0x10));
        assert_eq!(
            remote_string_field(&params, &layout, layout.cmdline_offset),
            Err(PebFieldError::InvalidUnicodeString)
        );
        // Buffer 为 0 → 字段不可得（None，非错误）。
        params[0x70..0x80].copy_from_slice(&us64(26, 32, 0));
        assert_eq!(remote_string_field(&params, &layout, layout.cmdline_offset), Ok(None));
    }

    #[test]
    fn validate_unicode_string_rules() {
        assert_eq!(validate_unicode_string(0, 10), Err(UnicodeStringError::Empty));
        assert_eq!(
            validate_unicode_string(u16::MAX, u16::MAX),
            Err(UnicodeStringError::Oversize)
        );
        assert_eq!(
            validate_unicode_string(100, 50),
            Err(UnicodeStringError::LengthExceedsMaximum)
        );
        assert_eq!(validate_unicode_string(26, 32), Ok(26));
        assert!(MAX_STRING_BYTES >= 32_768);
    }

    #[test]
    fn params_too_short_is_partial_result_signal() {
        let layout = PebLayout::win64();
        let short = vec![0u8; 0x60];
        assert_eq!(
            remote_string_field(&short, &layout, layout.cmdline_offset),
            Err(PebFieldError::ParamsTooShort {
                needed: 0x80,
                actual: 0x60,
            })
        );
    }

    #[test]
    fn environment_pointer_reads_64_and_32_bit() {
        let layout = PebLayout::win64();
        let mut params = vec![0u8; 136];
        params[0x80..0x88].copy_from_slice(&0x0000_7FF6_ABCD_9000u64.to_le_bytes());
        assert_eq!(
            environment_pointer(&params, &layout),
            Some(0x0000_7FF6_ABCD_9000)
        );

        let layout32 = PebLayout::wow64();
        let mut params32 = vec![0u8; 80];
        params32[72..76].copy_from_slice(&0x00A0_0000u32.to_le_bytes());
        assert_eq!(environment_pointer(&params32, &layout32), Some(0x00A0_0000));
    }

    #[test]
    fn env_limits_align_with_witr_bounded_read() {
        assert_eq!(MAX_ENV_BLOCK_BYTES, 128 * 1024);
        assert_eq!(ENV_CHUNK_BYTES, 4096);
    }
}