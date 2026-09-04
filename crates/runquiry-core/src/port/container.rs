//! 容器清单端口。

use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::model::capability::CapabilityStatus;
use crate::model::container_context::HealthcheckStatus;
use crate::model::error::InspectError;
use crate::model::ids::{ContainerKey, Pid, Port};
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

    /// 查找发布了指定主机端口的容器候选。
    ///
    /// 默认实现表达平台/运行时不支持该回退；Docker 适配器覆盖此方法。
    fn published_on(&self, port: Port) -> Inspection<Vec<ContainerSummary>> {
        Inspection::failed(vec![crate::DiagnosticIssue::new(
            crate::DiagnosticCode::Unsupported,
            format!("当前容器清单不支持按发布端口 {port} 查找"),
        )])
    }

    /// 读取运行时报告的宿主 PID 候选；运行时无法给出时返回 `Ok(None)`。
    ///
    /// 该值尚未经过归属验证，进入完整进程分析前必须交给
    /// [`ContainerProcessVerifier`]；验证失败时保留容器 fallback。
    fn host_pid(&self, key: &ContainerKey) -> Result<Option<Pid>, InspectError>;
}

/// 验证宿主进程是否仍属于目标容器的平台端口。
///
/// 实现必须读取传入 `pid` 自身的容器归属证据；证据缺失、不可读或不匹配均
/// 返回 `false`。该端口把 Linux cgroup 等平台细节留在 platform 层。
pub trait ContainerProcessVerifier {
    /// 验证候选宿主 PID 与容器键的归属关系。
    fn belongs_to_container(&self, pid: Pid, key: &ContainerKey) -> bool;
}

/// 仅当运行时返回的主机 PID 与调用方提供的 cgroup 证据属于同一容器时返回 PID。
///
/// cgroup 缺失或不包含非空容器 ID 时返回 `None`，避免把已复用的宿主 PID
/// 错配给容器。这里刻意不比较运行时名：清单键 `containerd` / `k8s` 与进程
/// 上下文 `nerdctl` / `crictl` 是 witr 中两套不同命名。
#[must_use]
pub fn verified_host_pid(
    candidate: Option<Pid>,
    key: &ContainerKey,
    cgroup: Option<&str>,
) -> Option<Pid> {
    let candidate = candidate?;
    let cgroup = cgroup?;
    (!key.id.is_empty() && cgroup.contains(&key.id)).then_some(candidate)
}

/// 容器健康检查定义探测端口（additive；parity §4 `ContainerHealthcheckStatus`）。
///
/// 判定的是容器**是否配置了 HEALTHCHECK**（`present` / `absent`），不是运行
/// 时健康状态。探测仅对 docker / podman 有意义（parity：其余运行时/平台不可
/// 判定），实现按 [`ContainerHealthcheckStatus::Present`] / `Absent` 返回；
/// 运行时不受支持、CLI 缺失或查询失败一律返回 `None`（对应 witr 空字符串，
/// 此时告警不触发）。
pub trait ContainerHealthcheckProbe {
    /// 探测容器是否定义了 HEALTHCHECK。
    ///
    /// 前置条件：`container_id` 为 cgroup 判出的容器 ID，`runtime` 为
    /// [`ContainerContext::runtime`](crate::model::container_context::ContainerContext::runtime)
    /// 语义的运行时标识；实现自行校验（非 docker/podman 返回 `None`）。
    fn healthcheck_status(&self, container_id: &str, runtime: &str) -> Option<HealthcheckStatus>;
}
