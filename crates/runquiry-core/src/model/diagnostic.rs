//! 结构化诊断：稳定错误码 + 面向用户的自由文本。

use serde::{Deserialize, Serialize};

/// 诊断类别码：稳定、可被 UI 与测试断言，承担控制流。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DiagnosticCode {
    /// 需要更高权限（读取受限的 `/proc` 条目等）。
    PermissionDenied,
    /// 依赖的外部命令执行失败（缺失、非零退出或超时）。
    ExternalToolFailed,
    /// 外部命令或采集调用超过时间上限。
    Timeout,
    /// 外部命令输出超过单次上限被截断。
    OutputLimitExceeded,
    /// 当前平台不存在该能力。
    Unsupported,
    /// 平台本应支持但当前环境不可用（运行时缺失、D-Bus 不可达）。
    PlatformUnavailable,
    /// 机器可读输出无法解析。
    ParseFailed,
    /// 未归类故障的兜底。
    Unknown,
}

impl DiagnosticCode {
    /// 稳定的 `snake_case` 错误码字符串。
    pub const fn code(self) -> &'static str {
        match self {
            Self::PermissionDenied => "permission_denied",
            Self::ExternalToolFailed => "external_tool_failed",
            Self::Timeout => "timeout",
            Self::OutputLimitExceeded => "output_limit_exceeded",
            Self::Unsupported => "unsupported",
            Self::PlatformUnavailable => "platform_unavailable",
            Self::ParseFailed => "parse_failed",
            Self::Unknown => "unknown",
        }
    }
}

/// 单条诊断：稳定错误码 + 面向用户的自由文本。
///
/// 自由文本仅用于展示，不承担控制流；分支判断必须使用 [`DiagnosticIssue::code`]。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticIssue {
    /// 稳定错误码。
    code: DiagnosticCode,
    /// 面向用户的说明文字，仅用于展示。
    message: String,
}

impl DiagnosticIssue {
    /// 构造一条诊断。
    pub const fn new(code: DiagnosticCode, message: String) -> Self {
        Self { code, message }
    }

    /// 读取稳定错误码。
    pub const fn code(&self) -> DiagnosticCode {
        self.code
    }

    /// 读取面向用户的说明文字。
    pub fn message(&self) -> &str {
        &self.message
    }
}
