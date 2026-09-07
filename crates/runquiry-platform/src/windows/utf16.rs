//! UTF-16 缓冲区的纯解码与环境块解析（无 OS 依赖）。
//!
//! 本文件只依赖 `std`，可在任何平台编译；Linux 上经
//! `#[path = "../src/windows/utf16.rs"]` 由 `tests/windows_*.rs` 直接执行。
//! 语义对齐 witr `peb_windows.go` 的 `parseEnvBlock` / `envBlockEnd`：
//! 环境块由 `KEY=VALUE\0` 条目组成，以空条目（连续两个 `\0`）终止；
//! 无终止符时按已读长度截断，不视为失败。

/// 单个远程 UNICODE_STRING 缓冲区的字节上限（32 KiB；命令行 / 路径的实际
/// 上限远低于此，超出按损坏数据处理）。
pub const MAX_STRING_BYTES: u32 = 32_768;
/// 远程环境块的总读取上限（witr `readEnvironmentBlock` 同值：128 KiB）。
pub const MAX_ENV_BLOCK_BYTES: usize = 128 * 1024;
/// 环境块单次远程读取块长（witr 同值：4 KiB）。
pub const ENV_CHUNK_BYTES: usize = 4096;
/// 解析以 NUL 终止的指针字符串时的扫描上限（UTF-16 单元数；SCM 名称实际
/// 上限 256 字符，超出即视为缓冲区损坏）。
pub const NUL_STRING_MAX_UNITS: usize = 4096;

/// 将 UTF-16LE 字节序列无损解码为字符串。
///
/// 尾部奇数字节（PEB 读取中字符串被改短的典型痕迹）被丢弃；成对代理项
/// 正常解码，孤立代理项由 `String::from_utf16_lossy` 替换为 U+FFFD。
#[must_use]
pub fn decode_lossy(bytes: &[u8]) -> String {
    decode_lossy_units(&le_units(bytes))
}

/// 将 UTF-16 单元序列解码为字符串（孤立代理项 → U+FFFD）。
#[must_use]
pub fn decode_lossy_units(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

/// 将 UTF-16LE 字节序列按小端读取为 u16 单元；尾字节不足 2 字节时丢弃。
fn le_units(bytes: &[u8]) -> Vec<u16> {
    bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect()
}

/// 在固定长度 UTF-16 数组（如 `PROCESSENTRY32W.szExeFile`）内读取首个
/// NUL 之前的字符串（缺失终止符时取全数组，调用方数组本身有界）。
#[must_use]
pub fn decode_fixed_array(units: &[u16]) -> String {
    let end = units.iter().position(|unit| *unit == 0).unwrap_or(units.len());
    decode_lossy_units(&units[..end])
}

/// 扫描 NUL 终止的 UTF-16 字符串；超过 `cap` 仍无终止符视为缓冲区损坏。
///
/// # Errors
/// 在 `cap` 内未找到终止符时返回 [`Utf16Error::NoTerminator`]。
pub fn nul_terminated_bounded(units: &[u16], cap: usize) -> Result<String, Utf16Error> {
    let end = units
        .iter()
        .take(cap)
        .position(|unit| *unit == 0)
        .ok_or(Utf16Error::NoTerminator)?;
    Ok(decode_lossy_units(&units[..end]))
}

/// UTF-16 解析错误（稳定文案，不携带任何进程数据）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Utf16Error {
    /// 在扫描上限内未找到 NUL 终止符（缓冲区越界或损坏）。
    NoTerminator,
}

/// 返回环境块内 `KEY=VALUE\0\0` 双 NUL 终止符的起始单元下标；未读到为 `None`。
#[must_use]
pub fn scan_env_block_end(block: &[u16]) -> Option<usize> {
    (0..block.len().saturating_sub(1)).find(|&index| block[index] == 0 && block[index + 1] == 0)
}

/// 环境块解析结果：键值对与「是否读到终止符」。
///
/// `terminated == false` 表示读取在达到块长上限或读取失败时截断——按已读
/// 长度解析已有内容，不伪造完整块（parity：无终止符按已读长度截断）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedEnv {
    /// `KEY=VALUE` 键值对；无 `=` 的损坏条目被跳过。
    pub pairs: Vec<(String, String)>,
    /// 是否读取到空条目终止符。
    pub terminated: bool,
}

/// 解析环境块单元序列（witr `parseEnvBlock` 的键值对化）。
#[must_use]
pub fn parse_env_block(block: &[u16]) -> ParsedEnv {
    let terminated = scan_env_block_end(block).is_some();
    let usable = match scan_env_block_end(block) {
        Some(end) => &block[..end],
        None => block,
    };
    let pairs = usable
        .split(|unit| *unit == 0)
        .filter(|entry| !entry.is_empty())
        .filter_map(|entry| {
            let text = decode_lossy_units(entry);
            let (key, value) = text.split_once('=')?;
            (!key.is_empty()).then(|| (String::from(key), String::from(value)))
        })
        .collect();
    ParsedEnv {
        pairs,
        terminated,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ParsedEnv, decode_fixed_array, decode_lossy, decode_lossy_units, nul_terminated_bounded,
        parse_env_block, scan_env_block_end, Utf16Error, NUL_STRING_MAX_UNITS,
    };

    #[test]
    fn decodes_ascii_utf16le_bytes() {
        let bytes: Vec<u8> = "C:\\opt\\runquiry-fixtures\\fxt-app.exe"
            .encode_utf16()
            .flat_map(|unit| unit.to_le_bytes())
            .collect();
        assert_eq!(decode_lossy(&bytes), "C:\\opt\\runquiry-fixtures\\fxt-app.exe");
    }

    #[test]
    fn odd_trailing_byte_is_dropped() {
        let mut bytes = Vec::new();
        bytes.extend(u16::from(0x41_u8).to_le_bytes());
        bytes.push(0x41);
        assert_eq!(decode_lossy(&bytes), "A");
    }

    #[test]
    fn non_bmp_char_decodes_from_paired_surrogates() {
        let emoji: Vec<u16> = "fxt\u{1F41B}".encode_utf16().collect();
        assert_eq!(decode_lossy_units(&emoji), "fxt\u{1F41B}");
    }

    #[test]
    fn lone_surrogate_becomes_replacement_char() {
        // 0xD800 是孤立高位代理项（无配对低位代理项）。
        assert_eq!(decode_lossy_units(&[0xD800]), "\u{FFFD}");
        assert_eq!(decode_lossy_units(&[0xDC00]), "\u{FFFD}");
    }

    #[test]
    fn fixed_array_stops_at_first_nul() {
        let mut units = "fxt-proc.exe".encode_utf16().collect::<Vec<_>>();
        units.push(0);
        units.extend([0x4141; 8]);
        assert_eq!(decode_fixed_array(&units), "fxt-proc.exe");
    }

    #[test]
    fn nul_terminated_bounded_finds_string() -> Result<(), Box<dyn std::error::Error>> {
        let mut units = "fxt-service".encode_utf16().collect::<Vec<_>>();
        units.push(0);
        units.extend([0x4141; 8]);
        assert_eq!(
            nul_terminated_bounded(&units, NUL_STRING_MAX_UNITS)
                .map_err(|error| format!("合成数据应含终止符：{error:?}"))?,
            "fxt-service"
        );
        Ok(())
    }

    #[test]
    fn nul_terminated_bounded_rejects_missing_terminator() {
        let units = "fxt-service".encode_utf16().collect::<Vec<_>>();
        assert_eq!(
            nul_terminated_bounded(&units, NUL_STRING_MAX_UNITS),
            Err(Utf16Error::NoTerminator)
        );
    }

    #[test]
    fn env_block_end_finds_double_nul() {
        let block: Vec<u16> = "A=V\0\0\x4141"
            .encode_utf16()
            .collect();
        assert_eq!(scan_env_block_end(&block), Some(3));
        assert_eq!(scan_env_block_end(&[0, 0]), Some(0));
        assert_eq!(scan_env_block_end(&[0]), None);
    }

    #[test]
    fn env_block_parses_pairs_and_stops_at_terminator() {
        let mut block = "FXT_VAR=fixture-value"
            .encode_utf16()
            .collect::<Vec<_>>();
        block.push(0);
        block.extend("FXT_NEXT=1".encode_utf16());
        block.push(0);
        block.push(0);
        block.extend([0x4141; 4]);
        let parsed = parse_env_block(&block);
        assert_eq!(
            parsed,
            ParsedEnv {
                pairs: vec![
                    (String::from("FXT_VAR"), String::from("fixture-value")),
                    (String::from("FXT_NEXT"), String::from("1")),
                ],
                terminated: true,
            }
        );
    }

    #[test]
    fn env_block_without_terminator_is_truncated_not_failed() {
        let mut block = "FXT_VAR=fixture-value".encode_utf16().collect::<Vec<_>>();
        block.push(0);
        block.extend("FXT_HALF".encode_utf16());
        let parsed = parse_env_block(&block);
        assert_eq!(parsed.terminated, false);
        assert_eq!(parsed.pairs.len(), 1);
    }

    #[test]
    fn env_block_skips_entries_without_equals_sign() {
        let mut block = "FXT_OK=1".encode_utf16().collect::<Vec<_>>();
        block.push(0);
        block.extend("NO_EQUALS_SIGN".encode_utf16());
        block.push(0);
        block.push(0);
        let parsed = parse_env_block(&block);
        assert_eq!(
            parsed.pairs,
            vec![(String::from("FXT_OK"), String::from("1"))]
        );
    }
}