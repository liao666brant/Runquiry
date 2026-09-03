//! 文件锁与打开文件端口。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::capability::CapabilityStatus;
use crate::model::ids::Pid;
use crate::model::inspection::Inspection;

/// 锁类型（parity：POSIX / FLOCK / OFDLCK）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LockType {
    /// POSIX 记录锁（`fcntl`）。
    Posix,
    /// BSD 文件锁（`flock`）。
    Flock,
    /// Open file description 锁。
    Ofdlck,
    /// 平台报告的其他锁类型，原样保留。
    Other,
}

/// 锁模式（parity：READ / WRITE / RW）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LockMode {
    /// 共享读锁。
    Read,
    /// 排他写锁。
    Write,
    /// 同时具备读与写。
    ReadWrite,
}

/// 文件锁条目（parity：`LockedFile{PID, Process, Path, Type, Mode}`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileLockEntry {
    /// 持有者进程 ID。
    pub pid: Pid,
    /// 持有者进程名。
    pub process: String,
    /// 被持有或打开的路径。
    pub path: PathBuf,
    /// 锁类型。
    pub lock_type: LockType,
    /// 锁模式。
    pub mode: LockMode,
}

/// 文件锁与打开文件采集端口。
///
/// 前置条件：`path` 为调用方给出的目标路径，实现可做符号链接归一化后比对。
/// 后置条件：无进程持有时返回空列表（调用方再决定是否报
/// [`InspectError::NotFound`](crate::model::error::InspectError::NotFound)）；
/// Windows 平台通过 [`CapabilityStatus::Unsupported`](crate::model::capability::CapabilityStatus::Unsupported)
/// 表达不可用，不返回伪数据。
pub trait FileInventory {
    /// 该能力的平台可用状态。
    fn capability(&self) -> CapabilityStatus;

    /// 查询持有指定文件（或对该文件加锁）的进程。
    fn holders(&self, path: &Path) -> Inspection<Vec<FileLockEntry>>;
}
