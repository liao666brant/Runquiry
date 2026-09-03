//! 进程基线、进程详情与进程控制端口。

use crate::model::capability::CapabilityStatus;
use crate::model::error::InspectError;
use crate::model::inspection::Inspection;
use crate::model::process::{ProcessAction, ProcessDetails, ProcessIdentity, ProcessSummary};

/// 进程基线清单：枚举当前可见进程的最小快照。
///
/// 后置条件：实现以真实采集结果填充 [`Inspection`]；单条目读取失败只追加
/// [`DiagnosticIssue`](crate::model::diagnostic::DiagnosticIssue)，不得丢弃其余条目。
pub trait ProcessInventory {
    /// 该能力的平台可用状态。
    fn capability(&self) -> CapabilityStatus;

    /// 采集进程基线快照。
    fn list(&self) -> Inspection<Vec<ProcessSummary>>;
}

/// 进程详情提供者：读取单个进程的深度信息。
///
/// 前置条件：`identity` 来自最近一次基线快照；实现发现 PID 已退出或被复用时，
/// 返回 [`InspectError::NotFound`] / [`InspectError::ProcessChanged`]，不返回旧数据。
pub trait ProcessDetailsProvider {
    /// 该能力的平台可用状态。
    fn capability(&self) -> CapabilityStatus;

    /// 读取指定身份的进程详情。
    fn details(&self, identity: &ProcessIdentity) -> Result<ProcessDetails, InspectError>;
}

/// 进程控制端口：执行两步确认后的进程操作。
///
/// 前置条件：`identity` 必须是执行前重读得到的身份；实现必须在执行动作前用
/// [`ProcessIdentity::same_process`] 校验重读身份与操作前快照一致，否则返回
/// [`InspectError::ProcessChanged`]（PID 复用防护）。重读得到的 `start_time`
/// 为 `None` 时身份不可验证，`same_process` 返回 `false`，实现必须拒绝执行，
/// 不得在身份不可证实的条件下发出信号或改优先级。
///
/// 后置条件：权限不足返回 [`InspectError::PermissionDenied`]，应用不自动提权；
/// Windows 平台通过 [`CapabilityStatus::Unsupported`](crate::model::capability::CapabilityStatus::Unsupported)
/// 表达整类动作不可用，而不是在执行时返回伪结果。
pub trait ProcessController {
    /// 该能力的平台可用状态。
    fn capability(&self) -> CapabilityStatus;

    /// 对指定身份执行动作。
    fn execute(
        &self,
        identity: &ProcessIdentity,
        action: ProcessAction,
    ) -> Result<(), InspectError>;
}
