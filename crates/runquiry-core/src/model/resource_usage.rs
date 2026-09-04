//! verbose 模式附加的资源明细模型（parity §1 `MemoryInfo` / `IOStats`）。

use serde::{Deserialize, Serialize};

/// 详细内存信息（parity：`MemoryInfo`，单位字节）。
///
/// 仅 verbose 详情采集填充；字段公开，平台按 `/proc` statm 等来源填写，
/// 不可得的字段记 0（witr 零值约定）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MemoryInfo {
    /// 虚拟内存总量（VMS，字节）。
    pub vms_bytes: u64,
    /// 常驻内存（RSS，字节）。
    pub rss_bytes: u64,
    /// 共享内存（字节）。
    pub shared_bytes: u64,
    /// 文本段（Text，字节）。
    pub text_bytes: u64,
    /// 共享库（Lib，字节）。
    pub lib_bytes: u64,
    /// 数据段（Data，字节）。
    pub data_bytes: u64,
    /// 脏页（Dirty，字节）。
    pub dirty_bytes: u64,
}

/// 进程 I/O 统计（parity：`IOStats{ReadBytes, WriteBytes, ReadOps, WriteOps}`；
/// Linux `/proc/PID/io`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct IoStats {
    /// 读取字节数。
    pub read_bytes: u64,
    /// 写入字节数。
    pub write_bytes: u64,
    /// 读操作次数。
    pub read_ops: u64,
    /// 写操作次数。
    pub write_ops: u64,
}
