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

    /// 读取指定身份的进程详情；可选字段失败时保留数据并追加结构化诊断。
    fn details(
        &self,
        identity: &ProcessIdentity,
    ) -> Result<Inspection<ProcessDetails>, InspectError>;
}

/// 进程控制端口：执行两步确认后的进程操作。
///
/// 身份参数语义：`execute` 的 `identity` 是**确认流程持有的 expected 快照**——
/// 即用户在确认对话框中看到并批准的那个身份，来自最近一次基线/详情快照；
/// 它不是实现重读的结果。不引入第二个调用方身份参数：expected 与 current 的
/// 比较由实现内部完成，调用方只传入确认流程的快照。
///
/// 前置条件（PID 复用防护，parity：`pidIdentityChanged` 比较 PID + `StartedAt`）：
/// * 实现必须在执行动作前按 `identity.pid()` 重读 current identity，再用
///   [`ProcessIdentity::same_process`] 比较 expected 与 current；
/// * 比较不一致（含重读得到的 `start_time` 为 `None`，即 current 身份不可验证、
///   `same_process` 恒为 `false`）时返回 [`InspectError::ProcessChanged`]，且
///   **不得产生任何副作用**：不发出信号、不改优先级、不计数成功动作；
///   `ProcessChanged` 携带重读得到的 current 身份供 UI 呈现。
///
/// 后置条件：权限不足返回 [`InspectError::PermissionDenied`]，应用不自动提权；
/// 平台可实现单个动作类别的子集：不可用的动作经 [`ProcessController::
/// action_capability`] 表达 `Unsupported`，UI 不得为其渲染入口。
///
/// `KillTree` 语义（Runquiry 扩展，无 witr 参照）：目标进程按上述身份校验
/// 执行强杀；后代自确认后的快照收集，逐个以快照 `start_time` 做 PID 复用
/// 防护后强杀（SIGKILL / TerminateProcess）。目标先于后代；目标强杀失败
/// 立即返回该错误。后代已退出（pidfd `ESRCH` / 打开报「消失或参数非法」）
/// 视为成功；身份不可验证（`start_time` 缺失或不匹配，含 PID 复用）跳过；
/// 其余失败——含**打开/信号阶段的权限拒绝**（受保护后代）——在全部尝试后
/// 聚合返回首个非「已退出」错误，使「部分后代存活」不至于被静默报成成功。
/// 后代集合中含 Runquiry 自身时在**任何后代被杀之前**整体返回
/// [`InspectError::InvalidTarget`]（不做部分清杀；目标本身已先行强杀）。
pub trait ProcessController {
    /// 该能力的平台可用状态（动作类别的整体可用性）。
    fn capability(&self) -> CapabilityStatus;

    /// 单个动作的可用状态；默认与整体能力一致。实现只对实际支持的动作
    /// 返回 `Supported`，UI 据此隐藏不支持的入口（如 Windows 的
    /// 暂停/恢复/renice）。
    fn action_capability(&self, action: &ProcessAction) -> CapabilityStatus {
        let _ = action;
        self.capability()
    }

    /// 以确认流程持有的 expected 身份执行动作（实现内部重读比对，见 trait 文档）。
    fn execute(
        &self,
        identity: &ProcessIdentity,
        action: ProcessAction,
    ) -> Result<(), InspectError>;

    /// 可执行文件定位（展示类扩展，非破坏性动作）的平台可用状态。
    ///
    /// 与 [`Self::action_capability`] 平行但独立：定位不修改进程状态，不走
    /// 两步确认流程。默认 `Unsupported`——只有提供系统文件管理器集成的平台
    /// 才覆盖（当前为 Windows）。
    fn reveal_capability(&self) -> CapabilityStatus {
        CapabilityStatus::Unsupported(String::from("平台不提供可执行文件定位"))
    }

    /// 在系统文件管理器中定位该进程的可执行文件（展示类扩展，非破坏性）。
    ///
    /// 语义：解析该进程的可执行路径（优先实时查询，失败回退调用方持有的快照
    /// 路径），交由平台文件管理器选中展示；两者皆不可得时返回
    /// [`InspectError::NotFound`]。定位为只读展示，不做身份重读比对——进程已
    /// 退出但快照路径可得时仍定位该路径。
    ///
    /// # Errors
    /// 平台不支持返回 [`InspectError::Unsupported`]；路径不可得返回
    /// [`InspectError::NotFound`]；路径存在但系统拒绝访问返回
    /// [`InspectError::PermissionDenied`]；文件管理器调用本身失败返回
    /// [`InspectError::ExternalTool`]（携带底层错误码）。
    fn reveal_executable(&self, identity: &ProcessIdentity) -> Result<(), InspectError> {
        let _ = identity;
        Err(InspectError::Unsupported {
            reason: String::from("平台不提供可执行文件定位"),
        })
    }
}
