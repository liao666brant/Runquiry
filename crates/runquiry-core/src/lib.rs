//! Runquiry 领域模型、目标解析、分析管线与平台端口。
//!
//! 本 crate 是 Runquiry 的纯领域层：不依赖 GPUI，不依赖操作系统实现，
//! 不引入 async runtime。平台差异通过 [`port`] 中的同步 trait 抽象，
//! 由 `runquiry-platform` 提供实现；行为语义的唯一来源是仓库根目录的
//! `docs/witr-parity.md`（witr 行为契约）。
//!
//! 已交付公共领域类型（`model`）、平台端口（`port`）、纯刷新状态机
//! （`refresh`）、目标解析与匹配（`resolution` / `resolve`）、祖先链
//! （`ancestry`）、来源识别（`source_detect`）、告警规则（`warnings`）
//! 与分析管线（`analyze`）。

pub mod analyze;
pub mod ancestry;
pub mod model;
pub mod port;
pub mod refresh;
pub mod resolution;
pub mod resolve;
pub mod source_detect;
pub mod source_shell;
pub mod warnings;

pub use analyze::{Analysis, AnalysisPorts, analyze};
pub use ancestry::resolve_ancestry;
pub use model::capability::CapabilityStatus;
pub use model::container_context::{
    ContainerContext, HealthcheckStatus, detect_container_from_cgroup, detect_lxc_runtime,
    find_long_hex_id, service_unit_from_cgroup, short_id, systemd_unit_from_cgroup,
};
pub use model::diagnostic::{DiagnosticCode, DiagnosticIssue};
pub use model::error::InspectError;
pub use model::health::HealthStatus;
pub use model::ids::{ContainerKey, InvalidId, Pid, Port};
pub use model::inspection::Inspection;
pub use model::process::{
    ProcessAction, ProcessDetails, ProcessIdentity, ProcessSummary, Renice, ReniceOutOfRange,
};
pub use model::resource_usage::{IoStats, MemoryInfo};
pub use model::source::{Source, SourceType};
pub use model::target::QueryTarget;
pub use port::command::{
    CommandOutput, CommandRunner, CommandSpec, DETAIL_TIMEOUT, LIST_TIMEOUT, PROBE_TIMEOUT,
    STDERR_LIMIT_BYTES, STDOUT_LIMIT_BYTES,
};
pub use port::container::{ContainerHealthcheckProbe, ContainerInventory, ContainerSummary};
pub use port::file::{FileInventory, FileLockEntry, LockMode, LockType, ProcessFileLocks};
pub use port::network::{
    NetworkInventory, OpenPortEntry, Protocol, SocketEntry, validate_socket_entries,
    validate_socket_entry,
};
pub use port::process::{ProcessController, ProcessDetailsProvider, ProcessInventory};
pub use port::source::{SourceEvidence, SourceEvidenceProvider};
pub use refresh::{Generation, RefreshGate};
pub use resolution::{
    Resolution, matches_exact_token, matches_fuzzy, parse_file_path, parse_pid, parse_port,
    parse_query,
};
pub use resolve::{
    ContainerMatchInput, merge_service_pid, resolve_containers, resolve_file_holders, resolve_name,
    resolve_port_owner, resolve_port_owner_in_sockets, scan_name_candidates,
};
pub use source_detect::detect_source;
pub use warnings::{Warning, WarningKind, WarningsContext, warnings};
