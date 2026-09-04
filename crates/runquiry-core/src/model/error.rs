//! 调查错误 `InspectError`：稳定错误码 + 面向用户的 Display。

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::model::process::ProcessIdentity;

/// 调查失败时的类型化错误（替代 witr 的字符串嗅探与数字退出码）。
///
/// 每个变体都有稳定错误码（见 [`InspectError::code`]）与面向用户的 `Display`；
/// `Display` 文本仅用于展示，不承担控制流。
///
/// 不实现 `PartialEq`：[`InspectError::ProcessChanged`] 携带
/// [`ProcessIdentity`]，其身份判定必须经由
/// [`ProcessIdentity::same_process`]（`start_time` 为 `None` 时不可验证），
/// 逐字段值比较会掩盖 PID 复用。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InspectError {
    /// 目标本身非法（如 PID 为 0、端口越界、路径为空）。
    InvalidTarget {
        /// 非法原因说明。
        reason: String,
    },
    /// 目标存在性未命中（无该进程 / 端口无属主 / 无进程持有文件 / 无匹配容器）。
    NotFound {
        /// 未命中的目标描述。
        subject: String,
    },
    /// 多结果命中，需进入候选表，不自动选择。
    ///
    /// 本变体只作错误信号与计数；完整候选明细不经错误承载——解析层把候选集合
    /// 作为正常返回值（如 `Vec<Candidate>`）交给 UI 呈现候选表（parity：多结果
    /// 返回 Ambiguous 和完整候选，不自动选择第一项）。
    Ambiguous {
        /// 命中过多的目标描述。
        subject: String,
        /// 候选数量；候选明细由调用方从采集数据中取得后交给 UI。
        candidate_count: usize,
    },
    /// 权限不足。应用不自动提权，只返回可操作的错误说明。
    PermissionDenied {
        /// 被拒绝访问的对象描述。
        subject: String,
    },
    /// 当前平台不支持该目标或能力（如 Windows 上的文件目标）。
    Unsupported {
        /// 不支持的原因说明。
        reason: String,
    },
    /// 依赖的外部命令失败（缺失、超时、非零退出或输出超限）。
    ExternalTool {
        /// 外部程序名。
        program: String,
        /// 失败细节（退出状态、超时等）。
        detail: String,
    },
    /// PID 被复用或进程已退出：破坏性操作前重读身份发现不一致，必须立即中止。
    ProcessChanged {
        /// 重读得到的新身份，与操作前快照比对。
        identity: ProcessIdentity,
    },
    /// 端口上存在 socket 但属主不可知（parity 哨兵
    /// `ErrSocketOwnerUnknown`：无权限或属主已退出，socket 本身存在）。
    /// 调用方可据此走容器回退（按端口查容器）或提示权限不足。
    SocketOwnerUnknown {
        /// 不可知属主的端口描述。
        subject: String,
    },
}

impl InspectError {
    /// 稳定的 `snake_case` 错误码。
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidTarget { .. } => "invalid_target",
            Self::NotFound { .. } => "not_found",
            Self::Ambiguous { .. } => "ambiguous",
            Self::PermissionDenied { .. } => "permission_denied",
            Self::Unsupported { .. } => "unsupported",
            Self::ExternalTool { .. } => "external_tool",
            Self::ProcessChanged { .. } => "process_changed",
            Self::SocketOwnerUnknown { .. } => "socket_owner_unknown",
        }
    }
}

impl fmt::Display for InspectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTarget { reason } => write!(f, "目标非法：{reason}"),
            Self::NotFound { subject } => write!(f, "未找到匹配的 {subject}"),
            Self::Ambiguous {
                subject,
                candidate_count,
            } => {
                write!(f, "{subject} 匹配到 {candidate_count} 个候选，需要人工选择")
            }
            Self::PermissionDenied { subject } => write!(f, "无权限访问 {subject}"),
            Self::Unsupported { reason } => write!(f, "当前平台不支持：{reason}"),
            Self::ExternalTool { program, detail } => {
                write!(f, "外部命令 {program} 失败：{detail}")
            }
            Self::ProcessChanged { identity } => {
                write!(f, "进程 {} 的身份已变化，请刷新后重试", identity.pid())
            }
            Self::SocketOwnerUnknown { subject } => {
                write!(f, "{subject} 存在 socket，但属主不可知（可能需要更高权限）")
            }
        }
    }
}

impl std::error::Error for InspectError {}
