//! 容器运行时的公共类型：快照条目、富集结果、CLI 名单与运行时种类。

use serde::{Deserialize, Serialize};

use runquiry_core::ContainerSummary;

/// 容器快照条目 + 解析阶段临时匹配键。
///
/// Compose 项目/服务键（`com.docker.compose.*` 标签）只随本结构返回，供目标
/// 解析阶段的匹配使用；**不得**进入 [`ContainerSummary`]、不得持久化、不得进 UI。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListedContainer {
    /// 容器快照（不含 Compose 临时键）。
    pub summary: ContainerSummary,
    /// Compose 项目临时匹配键（`com.docker.compose.project`）；仅解析阶段使用。
    pub compose_project: Option<String>,
    /// Compose 服务临时匹配键（`com.docker.compose.service`）；仅解析阶段使用。
    pub compose_service: Option<String>,
}

/// 富集结果：列表阶段没有、按容器补充的字段（对齐 witr `Enrich` 语义的子集）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerEnrichment {
    /// 容器主进程启动时间；运行时不提供时为 `None`。
    pub started_at: Option<std::time::SystemTime>,
}

/// 各运行时 CLI 的程序名或绝对路径（测试以假 CLI 路径覆盖；parity：LXD 需要
/// 客户端 `lxc` 与守护进程 `lxd` 同时存在，避免误入经典 LXC 的工具）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBinaries {
    /// `docker`。
    pub docker: String,
    /// `podman`。
    pub podman: String,
    /// `nerdctl`。
    pub nerdctl: String,
    /// `crictl`。
    pub crictl: String,
    /// `incus`。
    pub incus: String,
    /// LXD 客户端 `lxc`。
    pub lxd_client: String,
    /// LXD 守护进程 `lxd`。
    pub lxd_daemon: String,
    /// 经典 LXC 列表命令 `lxc-ls`。
    pub lxc_ls: String,
    /// 经典 LXC 信息命令 `lxc-info`。
    pub lxc_info: String,
}

impl Default for RuntimeBinaries {
    fn default() -> Self {
        Self {
            docker: String::from("docker"),
            podman: String::from("podman"),
            nerdctl: String::from("nerdctl"),
            crictl: String::from("crictl"),
            incus: String::from("incus"),
            lxd_client: String::from("lxc"),
            lxd_daemon: String::from("lxd"),
            lxc_ls: String::from("lxc-ls"),
            lxc_info: String::from("lxc-info"),
        }
    }
}

/// 运行时种类（B3 范围；FreeBSD jail 明确不实现）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeKind {
    Docker,
    Podman,
    Nerdctl,
    Crictl,
    Incus,
    Lxd,
    Lxc,
}

impl RuntimeKind {
    /// 全部运行时（列表顺序即汇总顺序）。
    pub(super) const ALL: [Self; 7] = [
        Self::Docker,
        Self::Podman,
        Self::Nerdctl,
        Self::Crictl,
        Self::Incus,
        Self::Lxd,
        Self::Lxc,
    ];

    /// [`ContainerKey::runtime`] 使用的运行时名（去重键组成部分；对齐 witr：
    /// crictl 为 `k8s`；nerdctl 采用 cgroup 语境的 `nerdctl`，显示名才是 containerd）。
    pub(super) const fn key_name(self) -> &'static str {
        match self {
            Self::Docker => "docker",
            Self::Podman => "podman",
            Self::Nerdctl => "nerdctl",
            Self::Crictl => "k8s",
            Self::Incus => "incus",
            Self::Lxd => "lxd",
            Self::Lxc => "lxc",
        }
    }

    /// 诊断与能力说明中的显示名（对齐 witr：nerdctl 显示 containerd、crictl 显示 k8s）。
    pub(super) const fn display_name(self) -> &'static str {
        match self {
            Self::Docker => "docker",
            Self::Podman => "podman",
            Self::Nerdctl => "containerd",
            Self::Crictl => "k8s",
            Self::Incus => "incus",
            Self::Lxd => "lxd",
            Self::Lxc => "lxc",
        }
    }
}
