//! Windows 缺失端口的 Unsupported 建模（纯委托 [`super::unsupported`]）。
//!
//! parity §10：Windows 使用文件共享模式而非 POSIX 式锁，无全系统锁枚举
//! API；文件锁整类表达为 [`CapabilityStatus::Unsupported`] 并返回固定失败
//! 结果——不返回伪数据、不经命令行模拟；UI 依据能力态先行禁用。容器归属
//! 验证恒 `false`（trait 契约：证据缺失不得用于归属判定）。
//! 进程控制已由 [`super::controller`] 实现关闭类（terminate/kill/kill-tree）。

use std::path::Path;

use runquiry_core::{
    CapabilityStatus, ContainerKey, ContainerProcessVerifier, DiagnosticCode, DiagnosticIssue,
    FileInventory, Inspection, Pid, ProcessFileLocks,
};

use super::WindowsPlatform;
use super::unsupported::{FILE_LOCKS_REASON, file_locks_failed_holders, file_locks_failed_list};

impl FileInventory for WindowsPlatform {
    fn capability(&self) -> CapabilityStatus {
        CapabilityStatus::Unsupported(String::from(FILE_LOCKS_REASON))
    }

    fn list(&self) -> Inspection<Vec<runquiry_core::FileInventoryEntry>> {
        file_locks_failed_list()
    }

    fn holders(&self, path: &Path) -> Inspection<Vec<runquiry_core::FileInventoryEntry>> {
        file_locks_failed_holders(path)
    }
}

impl ProcessFileLocks for WindowsPlatform {
    fn locks_of(&self, _pid: Pid) -> Inspection<Vec<runquiry_core::FileLockEntry>> {
        Inspection::failed(vec![DiagnosticIssue::new(
            DiagnosticCode::Unsupported,
            String::from(FILE_LOCKS_REASON),
        )])
    }
}

impl ContainerProcessVerifier for WindowsPlatform {
    /// Windows 无 cgroup 证据：恒 `false`（trait 契约：证据缺失返回 `false`）。
    ///
    /// Docker Desktop 的容器进程运行在 Linux 虚拟机内，其 PID 与宿主
    /// Windows 的 PID 空间互不可见，运行时报告的「host PID」不可映射、
    /// 不得用于调查或控制。
    fn belongs_to_container(&self, _pid: Pid, _key: &ContainerKey) -> bool {
        false
    }
}
