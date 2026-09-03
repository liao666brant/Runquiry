//! 平台能力四态表达。

use serde::{Deserialize, Serialize};

/// 平台能力状态：四态必须可被调用方明确区分，不得用伪数据或静默空集合表达不支持。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityStatus {
    /// 能力完整可用，实现必须返回真实数据。
    Supported,
    /// 能力可用但受限；`reason` 说明受限范围，供 UI 呈现。
    Partial(String),
    /// 当前平台没有该能力（如 Windows 上的文件锁与进程操作）；实现不得返回伪数据。
    Unsupported(String),
    /// 平台理论上有该能力但当前环境不可用（如 `lsof` 缺失、D-Bus 不可达）。
    Unavailable(String),
}

impl CapabilityStatus {
    /// 是否完整可用（仅 [`CapabilityStatus::Supported`] 为真）。
    pub const fn is_fully_supported(&self) -> bool {
        matches!(self, Self::Supported)
    }

    /// 是否至少能产生部分数据（`Supported` 与 `Partial`）。
    pub const fn is_usable(&self) -> bool {
        matches!(self, Self::Supported | Self::Partial(_))
    }

    /// 读取受限 / 不支持 / 不可用的原因；完整可用时为 `None`。
    pub const fn reason(&self) -> Option<&str> {
        match self {
            Self::Supported => None,
            Self::Partial(reason) | Self::Unsupported(reason) | Self::Unavailable(reason) => {
                Some(reason.as_str())
            }
        }
    }
}
