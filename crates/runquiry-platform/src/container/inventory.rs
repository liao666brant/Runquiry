//! 多运行时清单汇总、五字段解析与 Docker 发布端口回退。

use std::collections::HashSet;

use runquiry_core::{
    CapabilityStatus, ContainerInventory as InventoryPort, ContainerKey, ContainerMatchInput,
    ContainerSummary, DiagnosticCode, DiagnosticIssue, InspectError, Inspection, Pid, Port,
    Resolution, resolve_containers,
};

use super::ContainerRuntimes;
use super::runtime::{ListedContainer, RuntimeKind};

impl ContainerRuntimes {
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

    /// 以名称、镜像、入口命令或 Compose 项目/服务名解析容器。
    ///
    /// # Errors
    /// 全部运行时不可用、采集失败或没有候选命中时返回对应领域错误。
    pub fn resolve(
        &self,
        query: &str,
        exact: bool,
    ) -> Result<Inspection<Resolution<ContainerKey>>, InspectError> {
        let inspection = self.list_detailed();
        let Some(items) = inspection.data else {
            return Err(InspectError::ExternalTool {
                program: String::from("container-runtimes"),
                detail: inspection
                    .issues
                    .iter()
                    .map(DiagnosticIssue::message)
                    .collect::<Vec<_>>()
                    .join("；"),
            });
        };
        let candidates: Vec<_> = items
            .iter()
            .map(|entry| ContainerMatchInput {
                summary: &entry.summary,
                command: entry.command.as_deref(),
                compose_project: entry.compose_project.as_deref(),
                compose_service: entry.compose_service.as_deref(),
            })
            .collect();
        let resolution = resolve_containers(&candidates, query, exact)?;
        Ok(Inspection::partial(resolution, inspection.issues))
    }

    /// Docker 发布端口回退；只查询 Docker，不把其他运行时空结果误作命中。
    pub fn published_on(&self, port: Port) -> Inspection<Vec<ContainerSummary>> {
        if !self.probe(RuntimeKind::Docker) {
            return Inspection::failed(vec![DiagnosticIssue::new(
                DiagnosticCode::ExternalToolFailed,
                String::from("docker 运行时不可用：无法按发布端口查找容器"),
            )]);
        }
        match super::dockerlike::published_on(&self.docker_bin(), self.runner, port) {
            Ok((items, issues)) => Inspection::partial(
                items
                    .into_iter()
                    .map(ListedContainer::into_summary)
                    .collect(),
                issues,
            ),
            Err(issue) => Inspection::failed(vec![issue]),
        }
    }

    fn list_detailed(&self) -> Inspection<Vec<ListedContainer>> {
        let mut items = Vec::new();
        let mut issues = Vec::new();
        let mut seen = HashSet::new();
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
}

impl InventoryPort for ContainerRuntimes {
    fn capability(&self) -> CapabilityStatus {
        Self::capability(self)
    }

    fn list(&self) -> Inspection<Vec<ContainerSummary>> {
        self.list_detailed().map(|items| {
            items
                .into_iter()
                .map(ListedContainer::into_summary)
                .collect()
        })
    }

    fn published_on(&self, port: Port) -> Inspection<Vec<ContainerSummary>> {
        Self::published_on(self, port)
    }

    fn host_pid(&self, key: &ContainerKey) -> Result<Option<Pid>, InspectError> {
        self.host_pid_candidate(key)
    }
}

impl ListedContainer {
    fn into_summary(self) -> ContainerSummary {
        self.summary
    }
}
