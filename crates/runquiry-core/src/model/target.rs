//! 五类调查目标的显式枚举 `QueryTarget`。
//!
//! 语义来自 [witr 行为契约](../../../../docs/witr-parity.md) §2：
//! 调查入口使用显式目标类型，不自动猜测；`exact` 仅对名称与容器目标生效。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::model::ids::{Pid, Port};

/// 五类调查目标（名称 / PID / 端口 / 文件 / 容器）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryTarget {
    /// 按进程名调查：`exact` 为全等匹配，否则为大小写不敏感的子串匹配。
    ProcessName {
        /// 查询串。
        query: String,
        /// 是否要求完全相等。
        exact: bool,
    },
    /// 按进程 ID 调查（必须是正整数，忽略 `exact`）。
    Pid(Pid),
    /// 按端口号调查（必须在 1-65535，忽略 `exact`）。
    Port(Port),
    /// 按文件路径调查（解析与符号链接归一化由平台 `FileInventory` 承担）。
    File(PathBuf),
    /// 按容器调查：匹配名称、镜像、命令等字段；`exact` 为全等匹配。
    Container {
        /// 查询串。
        query: String,
        /// 是否要求字段完全相等。
        exact: bool,
    },
}

impl QueryTarget {
    /// 目标类型名（与 witr 的 `TargetType` 对应），供 UI 与日志使用。
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::ProcessName { .. } => "name",
            Self::Pid(_) => "pid",
            Self::Port(_) => "port",
            Self::File(_) => "file",
            Self::Container { .. } => "container",
        }
    }
}
