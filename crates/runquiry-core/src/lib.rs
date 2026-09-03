//! Runquiry 领域模型、目标解析、分析管线与平台端口。
//!
//! 本 crate 是 Runquiry 的纯领域层：不依赖 GPUI，不依赖操作系统实现，
//! 不引入 async runtime。平台差异通过 [`port`] 中的同步 trait 抽象，
//! 由 `runquiry-platform` 提供实现；行为语义的唯一来源是仓库根目录的
//! `docs/witr-parity.md`（witr 行为契约）。
//!
//! A3 交付范围：公共领域类型（`model`）与七个平台端口（`port`）。
//! 目标解析算法、匹配、来源识别、告警规则与分析管线属于 B1，尚未实现。

pub mod model;
pub mod port;
pub mod refresh;

pub use model::capability::CapabilityStatus;
pub use model::diagnostic::{DiagnosticCode, DiagnosticIssue};
pub use model::error::InspectError;
pub use model::ids::{ContainerKey, InvalidId, Pid, Port};
pub use model::inspection::Inspection;
pub use model::process::{
    ProcessAction, ProcessDetails, ProcessIdentity, ProcessSummary, Renice, ReniceOutOfRange,
};
pub use model::target::QueryTarget;
pub use port::command::{
    CommandOutput, CommandRunner, CommandSpec, DETAIL_TIMEOUT, LIST_TIMEOUT, PROBE_TIMEOUT,
    STDERR_LIMIT_BYTES, STDOUT_LIMIT_BYTES,
};
pub use port::container::{ContainerInventory, ContainerSummary};
pub use port::file::{FileInventory, FileLockEntry, LockMode, LockType};
pub use port::network::{NetworkInventory, OpenPortEntry, Protocol, SocketEntry};
pub use port::process::{ProcessController, ProcessDetailsProvider, ProcessInventory};
pub use refresh::{Generation, RefreshGate};
