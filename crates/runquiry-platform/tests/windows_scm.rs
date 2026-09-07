//! Windows SCM 缓冲区解析的 Linux 编译入口与行为测试（见
//! `windows_utf16.rs` 头注释）。
// utf16 的边界常量由 peb/fields 消费，本目标不含该模块，放行。
#![allow(dead_code)]

#[path = "../src/windows/utf16.rs"]
mod utf16;

#[path = "../src/windows/scm_parse.rs"]
mod scm_parse;

use scm_parse::{
    RawServiceEntry, ScmParseError, ServiceConfig, dedup_by_pid, enum_entry_stride,
    parse_enum_buffer, parse_query_service_config, parse_service_description, service_state_name,
};

/// 测试内错误传播：`ScmParseError` 未实现 `std::error::Error`（不为此改动
/// 生产公共 API），经 `Debug` 文案映射为 String 后 `?` 传播。
fn ok<T, E: std::fmt::Debug>(context: &str, result: Result<T, E>) -> Result<T, String> {
    result.map_err(|error| format!("{context}: {error:?}"))
}

/// 构造缓冲区：条目数组 + 尾部字符串区，字符串指针为 `base + offset`。
struct EnumFixture {
    buf: Vec<u8>,
    base: usize,
}

impl EnumFixture {
    fn new(count: u32) -> Self {
        let mut buf = vec![0u8; count as usize * enum_entry_stride()];
        buf.extend_from_slice(&[0u8; 16]);
        Self {
            base: 0x0000_7FF6_0000_0000,
            buf,
        }
    }

    fn push_string(&mut self, text: &str) -> usize {
        let offset = self.buf.len();
        for unit in text.encode_utf16() {
            self.buf.extend_from_slice(&unit.to_le_bytes());
        }
        self.buf.extend_from_slice(&[0, 0]);
        offset
    }

    fn set_pointer(&mut self, entry: usize, field: usize, offset: usize) {
        let width = size_of::<usize>();
        let at = entry * enum_entry_stride() + field;
        let value = self.base + offset;
        for (index, byte) in value.to_le_bytes().iter().take(width).enumerate() {
            self.buf[at + index] = *byte;
        }
    }
}

/// 写入一个条目的状态与 PID（status 区起始 = 2 × 指针宽度）。
fn set_status(fixture: &mut EnumFixture, entry: usize, state: u32, pid: u32) {
    let at = entry * enum_entry_stride() + size_of::<usize>() * 2;
    fixture.buf[at + 4..at + 8].copy_from_slice(&state.to_le_bytes());
    fixture.buf[at + 28..at + 32].copy_from_slice(&pid.to_le_bytes());
}

#[test]
fn enum_buffer_parses_entries_with_strings() -> Result<(), Box<dyn std::error::Error>> {
    let mut fixture = EnumFixture::new(1);
    let name = fixture.push_string("fxt-service");
    let display = fixture.push_string("Fixture Service");
    fixture.set_pointer(0, 0, name);
    fixture.set_pointer(0, size_of::<usize>(), display);
    set_status(&mut fixture, 0, 4, 6260);

    let entries = ok(
        "枚举缓冲区",
        parse_enum_buffer(&fixture.buf, fixture.base, 1),
    )?;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "fxt-service");
    assert_eq!(entries[0].display_name, "Fixture Service");
    assert_eq!(entries[0].state_raw, 4);
    assert_eq!(entries[0].pid_raw, 6260);
    Ok(())
}

#[test]
fn dedup_skips_stopped_and_empty_and_keeps_first_writer() {
    let entries = vec![
        RawServiceEntry {
            name: String::from("fxt-first"),
            display_name: String::from("First"),
            state_raw: 4,
            pid_raw: 100,
        },
        RawServiceEntry {
            name: String::from("fxt-second"),
            display_name: String::from("Second"),
            state_raw: 4,
            pid_raw: 100,
        },
        RawServiceEntry {
            name: String::from("fxt-stopped"),
            display_name: String::from("Stopped"),
            state_raw: 1,
            pid_raw: 0,
        },
        RawServiceEntry {
            name: String::new(),
            display_name: String::from("NoName"),
            state_raw: 4,
            pid_raw: 200,
        },
    ];
    let deduped = dedup_by_pid(entries);
    assert_eq!(deduped.len(), 1);
    assert_eq!(deduped[0].0, 100);
    assert_eq!(deduped[0].1.name, "fxt-first");
}

#[test]
fn truncated_count_is_typed_failure() {
    let buf = vec![0u8; 4];
    assert_eq!(
        parse_enum_buffer(&buf, 0x1000, 2),
        Err(ScmParseError::BufferTooShort {
            count: 2,
            needed: 2 * enum_entry_stride(),
            actual: 4,
        })
    );
}

#[test]
fn dangling_pointer_is_typed_failure() {
    let mut fixture = EnumFixture::new(1);
    // 指针悬空：指向缓冲区之外（base - 0x10）。
    let width = size_of::<usize>();
    let value = fixture.base - 0x10;
    for (index, byte) in value.to_le_bytes().iter().take(width).enumerate() {
        fixture.buf[index] = *byte;
    }
    set_status(&mut fixture, 0, 4, 100);
    assert_eq!(
        parse_enum_buffer(&fixture.buf, fixture.base, 1),
        Err(ScmParseError::PointerOutOfRange)
    );
}

#[test]
fn state_names_map_stably() {
    assert_eq!(service_state_name(4), "Running");
    assert_eq!(service_state_name(1), "Stopped");
    assert_eq!(service_state_name(7), "Paused");
    assert_eq!(service_state_name(0), "Unknown");
}

/// 构造 `QueryServiceConfigW` 缓冲区：64 字节结构 + 尾部路径字符串。
#[test]
fn query_service_config_parses_start_and_binary_path() -> Result<(), Box<dyn std::error::Error>> {
    let base = 0x0000_7FF6_0000_0000usize;
    let mut buf = vec![0u8; 64];
    buf[4..8].copy_from_slice(&2u32.to_le_bytes()); // SERVICE_AUTO_START
    let mut tail = "C:\\opt\\runquiry-fixtures\\fxt-app.exe"
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect::<Vec<u8>>();
    tail.extend_from_slice(&[0, 0]);
    let offset = buf.len();
    buf.extend_from_slice(&tail);
    let pointer = base + offset;
    for (index, byte) in pointer.to_le_bytes().iter().take(8).enumerate() {
        buf[16 + index] = *byte;
    }

    let config = ok("服务配置", parse_query_service_config(&buf, base))?;
    assert_eq!(
        config,
        ServiceConfig {
            start_raw: 2,
            binary_path: Some(String::from("C:\\opt\\runquiry-fixtures\\fxt-app.exe")),
        }
    );
    Ok(())
}

#[test]
fn query_service_config_with_bad_pointer_degrades_binary_path()
-> Result<(), Box<dyn std::error::Error>> {
    let base = 0x1000usize;
    let mut buf = vec![0u8; 64];
    buf[4..8].copy_from_slice(&3u32.to_le_bytes());
    let config = ok("服务配置", parse_query_service_config(&buf, base))?;
    assert_eq!(config.start_raw, 3);
    assert_eq!(config.binary_path, None, "路径不可得时保留其余字段");
    Ok(())
}

#[test]
fn service_description_parses_from_config2_buffer() -> Result<(), Box<dyn std::error::Error>> {
    let base = 0x0000_7FF6_0000_0000usize;
    let mut buf = vec![0u8; 8];
    let mut tail = "Fixture 服务描述"
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect::<Vec<u8>>();
    tail.extend_from_slice(&[0, 0]);
    let offset = buf.len();
    buf.extend_from_slice(&tail);
    for (index, byte) in (base + offset).to_le_bytes().iter().take(8).enumerate() {
        buf[index] = *byte;
    }
    assert_eq!(
        ok("服务描述", parse_service_description(&buf, base))?,
        "Fixture 服务描述"
    );
    Ok(())
}
