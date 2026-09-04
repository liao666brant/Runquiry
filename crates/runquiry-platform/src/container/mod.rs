//! 容器运行时适配器（B3）：七种运行时经 `CommandRunner` 汇总。
//!
//! Docker、Podman、nerdctl、crictl、Incus、LXC、LXD 各自独立实现
//! available / list / host PID / enrich；单运行时失败只追加诊断。
//! 子模块由本目录内代码自组织。

mod crictl;
mod dockerlike;
mod lxc;
mod lxdlike;
mod parse;
mod runtime;

use std::collections::HashSet;

use runquiry_core::{
    CapabilityStatus, CommandSpec, ContainerHealthcheckProbe, ContainerInventory as InventoryPort,
    ContainerKey, ContainerSummary, DiagnosticCode, DiagnosticIssue, HealthcheckStatus,
    InspectError, Inspection, Pid, port::command::PROBE_TIMEOUT,
};

use crate::command::StdCommandRunner;
use crate::container::runtime::RuntimeKind;

// 供外部（如测试 crate）直接引用的公共类型再导出。
pub use crate::container::runtime::{ContainerEnrichment, ListedContainer, RuntimeBinaries};

/// 七种容器运行时的汇总清单：单一运行时失败不阻断其他运行时（Inspection 部分成功）。
///
/// 跨运行时按 [`ContainerKey`]（runtime + id）去重；重复短 ID 在不同运行时下
/// 是不同容器，不合并。
#[derive(Debug)]
pub struct ContainerRuntimes {
    runner: StdCommandRunner,
    bins: RuntimeBinaries,
}

impl Default for ContainerRuntimes {
    fn default() -> Self {
        Self::new()
    }
}

impl ContainerRuntimes {
    /// 以默认 CLI 名与 [`StdCommandRunner`] 构造。
    pub fn new() -> Self {
        Self {
            runner: StdCommandRunner,
            bins: RuntimeBinaries::default(),
        }
    }

    /// 以自定义 CLI 程序名/路径构造（测试注入假 CLI；生产请用 [`Self::new`]）。
    pub const fn with_binaries(bins: RuntimeBinaries) -> Self {
        Self {
            runner: StdCommandRunner,
            bins,
        }
    }

    /// 容器能力状态：探测各运行时 CLI 可用性后汇总。
    pub fn capability(&self) -> CapabilityStatus {
        let mut available = Vec::new();
        let mut missing = Vec::new();
        for kind in RuntimeKind::ALL {
            if self.probe(kind) {
                available.push(kind.display_name());
            } else {
                missing.push(kind.display_name());
            }
        }
        if available.is_empty() {
            return CapabilityStatus::Unavailable(String::from(
                "未发现任何容器运行时 CLI（docker/podman/nerdctl/crictl/incus/lxd/lxc）",
            ));
        }
        if missing.is_empty() {
            return CapabilityStatus::Supported;
        }
        CapabilityStatus::Partial(format!(
            "可用运行时: {}；缺失: {}",
            available.join(", "),
            missing.join(", ")
        ))
    }

    /// 列出各可用运行时的容器（含 Compose 临时匹配键）。
    ///
    /// 失败隔离：任一运行时的探测失败 / 命令失败 / 超时 / 解析失败只产生
    /// [`DiagnosticIssue`]，其他运行时的结果照常返回；全部失败时 `data` 为 `None`。
    /// 同一运行时内与跨运行时均按 `runtime|id` 去重。
    pub fn list_detailed(&self) -> Inspection<Vec<ListedContainer>> {
        let mut items = Vec::new();
        let mut issues = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut any_success = false;
        for kind in RuntimeKind::ALL {
            if !self.probe(kind) {
                issues.push(DiagnosticIssue::new(
                    DiagnosticCode::ExternalToolFailed,
                    format!("{} 运行时不可用：CLI 缺失或无法执行", kind.display_name()),
                ));
                continue;
            }
            match self.list_one(kind) {
                Ok((list, extra_issues)) => {
                    any_success = true;
                    issues.extend(extra_issues);
                    for entry in list {
                        if seen.insert(entry.summary.key.dedup_key()) {
                            items.push(entry);
                        }
                    }
                }
                Err(issue) => issues.push(issue),
            }
        }
        if any_success {
            Inspection::partial(items, issues)
        } else {
            Inspection::failed(issues)
        }
    }

    /// 解析容器在主机上的进程 ID；运行时无法给出时返回 `Ok(None)`。
    ///
    /// 主机 PID 缺失、为零或为负一律 `None`，不伪造进程详情；"PID 是否真属于
    /// 容器" 的 cgroup 校验属 Linux 管线职责，不在本层。
    pub fn host_pid(&self, key: &ContainerKey) -> Result<Option<Pid>, InspectError> {
        let kind = Self::kind_for(&key.runtime)?;
        Self::require_valid_id(&key.id)?;
        match kind {
            RuntimeKind::Docker => dockerlike::host_pid(&self.docker_bin(), self.runner, &key.id),
            RuntimeKind::Podman => dockerlike::host_pid(&self.podman_bin(), self.runner, &key.id),
            RuntimeKind::Nerdctl => dockerlike::host_pid(&self.nerdctl_bin(), self.runner, &key.id),
            RuntimeKind::Crictl => crictl::host_pid(&self.bins.crictl, self.runner, &key.id),
            RuntimeKind::Incus => {
                lxdlike::host_pid(&self.bins.incus, self.runner, kind.display_name(), &key.id)
            }
            RuntimeKind::Lxd => lxdlike::host_pid(
                &self.bins.lxd_client,
                self.runner,
                kind.display_name(),
                &key.id,
            ),
            RuntimeKind::Lxc => lxc::host_pid(&self.bins.lxc_info, self.runner, &key.id),
        }
    }

    /// 富集容器：补充列表阶段没有的字段（如 docker-like 的启动时间）。
    ///
    /// Incus/LXD/LXC 可富集的网络与挂载不在本轮范围，返回空富集。
    pub fn enrich(&self, key: &ContainerKey) -> Result<ContainerEnrichment, InspectError> {
        let kind = Self::kind_for(&key.runtime)?;
        Self::require_valid_id(&key.id)?;
        match kind {
            RuntimeKind::Docker => dockerlike::enrich(&self.docker_bin(), self.runner, &key.id),
            RuntimeKind::Podman => dockerlike::enrich(&self.podman_bin(), self.runner, &key.id),
            RuntimeKind::Nerdctl => dockerlike::enrich(&self.nerdctl_bin(), self.runner, &key.id),
            RuntimeKind::Crictl => crictl::enrich(&self.bins.crictl, self.runner, &key.id),
            RuntimeKind::Incus | RuntimeKind::Lxd => Ok(lxdlike::enrich()),
            RuntimeKind::Lxc => Ok(lxc::enrich()),
        }
    }

    fn kind_for(runtime: &str) -> Result<RuntimeKind, InspectError> {
        RuntimeKind::ALL
            .into_iter()
            .find(|kind| kind.key_name() == runtime)
            .ok_or_else(|| InspectError::Unsupported {
                reason: format!("未知容器运行时：{runtime}"),
            })
    }

    /// 容器 ID 交给 CLI 前的安全校验（parity：isValidContainerID）。
    fn require_valid_id(id: &str) -> Result<(), InspectError> {
        if parse::is_valid_container_id(id) {
            Ok(())
        } else {
            Err(InspectError::InvalidTarget {
                reason: format!("容器 ID `{id}` 不是安全的 CLI 参数"),
            })
        }
    }

    fn docker_bin(&self) -> dockerlike::DockerLikeBin {
        dockerlike::DockerLikeBin {
            program: self.bins.docker.clone(),
            runtime: RuntimeKind::Docker.key_name(),
            list_format: "{{json .}}",
            line_delimited: true,
        }
    }

    fn podman_bin(&self) -> dockerlike::DockerLikeBin {
        dockerlike::DockerLikeBin {
            program: self.bins.podman.clone(),
            runtime: RuntimeKind::Podman.key_name(),
            list_format: "json",
            line_delimited: false,
        }
    }

    fn nerdctl_bin(&self) -> dockerlike::DockerLikeBin {
        dockerlike::DockerLikeBin {
            program: self.bins.nerdctl.clone(),
            runtime: RuntimeKind::Nerdctl.key_name(),
            list_format: "json",
            line_delimited: false,
        }
    }

    /// 可用性探测：运行一条只读、不触碰守护进程的 `--version` 命令
    /// （`PROBE_TIMEOUT）；能启动即视为可用（parity：witr` 只检查二进制存在）。
    fn probe(&self, kind: RuntimeKind) -> bool {
        match kind {
            RuntimeKind::Docker => self.probe_program(&self.bins.docker),
            RuntimeKind::Podman => self.probe_program(&self.bins.podman),
            RuntimeKind::Nerdctl => self.probe_program(&self.bins.nerdctl),
            RuntimeKind::Crictl => self.probe_program(&self.bins.crictl),
            RuntimeKind::Incus => self.probe_program(&self.bins.incus),
            // LXD 需客户端与守护进程二进制同时存在，避免误入经典 LXC 工具链。
            RuntimeKind::Lxd => {
                self.probe_program(&self.bins.lxd_client)
                    && self.probe_program(&self.bins.lxd_daemon)
            }
            RuntimeKind::Lxc => self.probe_program(&self.bins.lxc_ls),
        }
    }

    fn probe_program(&self, program: &str) -> bool {
        let spec = CommandSpec::new(program, ["--version"]);
        self.runner.run_classified(&spec, PROBE_TIMEOUT).is_ok()
    }

    fn list_one(
        &self,
        kind: RuntimeKind,
    ) -> Result<(Vec<ListedContainer>, Vec<DiagnosticIssue>), DiagnosticIssue> {
        match kind {
            RuntimeKind::Docker => dockerlike::list(&self.docker_bin(), self.runner),
            RuntimeKind::Podman => dockerlike::list(&self.podman_bin(), self.runner),
            RuntimeKind::Nerdctl => dockerlike::list(&self.nerdctl_bin(), self.runner),
            RuntimeKind::Crictl => crictl::list(&self.bins.crictl, self.runner),
            RuntimeKind::Incus => lxdlike::list(&self.bins.incus, self.runner, kind.key_name()),
            RuntimeKind::Lxd => lxdlike::list(&self.bins.lxd_client, self.runner, kind.key_name()),
            RuntimeKind::Lxc => lxc::list(&self.bins.lxc_ls, self.runner),
        }
    }
}

impl InventoryPort for ContainerRuntimes {
    fn capability(&self) -> CapabilityStatus {
        Self::capability(self)
    }

    fn list(&self) -> Inspection<Vec<ContainerSummary>> {
        // Compose 临时匹配键止步于解析阶段结构，不随 trait 快照外泄。
        self.list_detailed().map(|items| {
            items
                .into_iter()
                .map(ListedContainer::into_summary)
                .collect()
        })
    }

    fn host_pid(&self, key: &ContainerKey) -> Result<Option<Pid>, InspectError> {
        Self::host_pid(self, key)
    }
}

impl ContainerHealthcheckProbe for ContainerRuntimes {
    /// 探测容器是否定义了 HEALTHCHECK（parity：仅 docker/podman 可判定；
    /// 其余运行时、未知 runtime 或探测失败一律 `None`——「无健康检查」
    /// 告警不触发，与 witr 空串语义一致）。
    fn healthcheck_status(&self, container_id: &str, runtime: &str) -> Option<HealthcheckStatus> {
        let kind = RuntimeKind::ALL
            .into_iter()
            .find(|kind| kind.key_name() == runtime)?;
        if !matches!(kind, RuntimeKind::Docker | RuntimeKind::Podman) {
            return None;
        }
        Self::require_valid_id(container_id).ok()?;
        let bin = match kind {
            RuntimeKind::Docker => self.docker_bin(),
            RuntimeKind::Podman => self.podman_bin(),
            _ => return None,
        };
        dockerlike::healthcheck_config(&bin, self.runner, container_id)
            .ok()
            .flatten()
    }
}

impl ListedContainer {
    /// 取快照部分（丢弃 Compose 临时匹配键）。
    pub fn into_summary(self) -> ContainerSummary {
        self.summary
    }
}
