//! 容器清单端口。

use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::model::capability::CapabilityStatus;
use crate::model::error::InspectError;
use crate::model::ids::{ContainerKey, Pid};
use crate::model::inspection::Inspection;

/// 容器快照条目。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerSummary {
    /// 运行时 + ID 组成的唯一键（去重键语义）。
    pub key: ContainerKey,
    /// 容器名称。
    pub name: Option<String>,
    /// 镜像引用。
    pub image: Option<String>,
    /// 运行状态描述（如 `Up 2 hours`）。
    pub status: Option<String>,
    /// 健康检查结果；不可判定时为 `None`。
    pub health: Option<String>,
    /// 容器在主机上的进程 ID；运行时无法给出时为 `None`。
    pub host_pid: Option<Pid>,
    /// 容器启动时间。
    pub started_at: Option<SystemTime>,
}

/// 容器清单端口。
///
/// 前置条件：实现只调用真实存在且受支持的运行时 CLI，且全部外部命令经
/// [`CommandRunner`](crate::port::command::CommandRunner) 执行。
/// 后置条件：
/// * 跨运行时按 [`ContainerKey`] 去重，同一容器不重复出现；
/// * 某个运行时失败只追加 [`DiagnosticIssue`](crate::model::diagnostic::DiagnosticIssue)，
///   不得阻断其他运行时，也不得整体失败；
/// * 运行时缺失属于能力状态（[`CapabilityStatus`]），不是空集合。
pub trait ContainerInventory {
    /// 容器能力的平台可用状态。
    fn capability(&self) -> CapabilityStatus;

    /// 列出各可用运行时中的容器。
    fn list(&self) -> Inspection<Vec<ContainerSummary>>;

    /// 解析容器在主机上的进程 ID；运行时无法给出时返回 `Ok(None)`。
    fn host_pid(&self, key: &ContainerKey) -> Result<Option<Pid>, InspectError>;
}
