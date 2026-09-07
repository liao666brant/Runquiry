//! 最小 XML plist 字段提取（纯逻辑：只依赖 std）。
//!
//! 契约来自 witr `internal/launchd/plist.go`：plutil 已把 plist（含二进制）
//! 统一为 XML，这里只提取 root dict 层级上 witr 消费的字段——Label、Comment、
//! Program（string）、ProgramArguments / WatchPaths / QueueDirectories
//! （string 数组）、RunAtLoad / KeepAlive（bool）、StartInterval（integer）、
//! StartCalendarInterval（dict / 数组）。嵌套 dict 内的其他键一律忽略
//! （witr：`dictDepth > 1` 清空 currentKey；KeepAlive 为 dict 时按 false 处理）。
//!
//! 不追求完整 XML 规范：只识别 plist 用到的标签（dict/key/string/integer/
//! true/false/array），其余标签按事件跳过；文本实体只解码 XML 标准五实体与
//! 数字实体。
//!
//! 结构：[`reader`](self::reader)（标签流 → 事件流与实体还原）、
//! [`keys`](self::keys)（事件流 → 字段提取与人读文本）。本目录不依赖
//! macOS 系统库，Linux 上经 `#[path = "../src/macos/plist/mod.rs"]` 由
//! `tests/macos_launchd_parse.rs` 直接编译运行。

mod keys;
mod reader;

pub(crate) use keys::{LaunchdPlistInfo, format_triggers, parse_plist_xml};
