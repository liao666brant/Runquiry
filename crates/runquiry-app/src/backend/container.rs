//! 容器目标解析：可验证宿主进程，否则保留容器自身详情。

use runquiry_core::{
    ContainerInventory, ContainerSummary, DiagnosticCode, DiagnosticIssue, InspectError, Pid, Port,
    Resolution,
};
use runquiry_ui::backend::{ContainerInvestigation, InvestigationTarget};

use super::PlatformBackend;

impl PlatformBackend {
    pub(super) fn resolve_published_port_container(
        &self,
        port: Port,
        subject: String,
        inventory: &[runquiry_core::ProcessSummary],
        platform: &runquiry_platform::linux::LinuxPlatform,
    ) -> Result<Resolution<InvestigationTarget>, InspectError> {
        let published = self.containers.published_on(port);
        let Some(summaries) = published.data else {
            return Err(InspectError::SocketOwnerUnknown { subject });
        };
        let candidates = summaries
            .into_iter()
            .map(|summary| self.container_target(summary, inventory, &published.issues, platform))
            .collect();
        published_container_resolution(candidates, subject)
    }

    pub(super) fn resolve_container(
        &self,
        query: &str,
        exact: bool,
        platform: &runquiry_platform::linux::LinuxPlatform,
    ) -> Result<Resolution<InvestigationTarget>, InspectError> {
        let resolved = self.containers.resolve(query, exact)?;
        let mut issues = resolved.issues;
        let keys = resolved.data.ok_or_else(|| InspectError::NotFound {
            subject: format!("容器 {query:?}"),
        })?;
        let listed = ContainerInventory::list(self.containers.as_ref());
        issues.extend(listed.issues);
        let summaries = listed.data.ok_or_else(|| InspectError::Unsupported {
            reason: String::from("容器清单采集未返回数据"),
        })?;
        let inventory = Self::identities(platform)?;
        let candidates = keys
            .into_candidates()
            .into_iter()
            .filter_map(|key| summaries.iter().find(|item| item.key == key).cloned())
            .map(|summary| self.container_target(summary, &inventory, &issues, platform))
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [] => Err(InspectError::NotFound {
                subject: format!("容器 {query:?}"),
            }),
            [candidate] => Ok(Resolution::Unique(candidate.clone())),
            _ => Ok(Resolution::Ambiguous(candidates)),
        }
    }

    fn container_target(
        &self,
        summary: ContainerSummary,
        inventory: &[runquiry_core::ProcessSummary],
        inherited_issues: &[DiagnosticIssue],
        platform: &runquiry_platform::linux::LinuxPlatform,
    ) -> InvestigationTarget {
        let mut issues = inherited_issues.to_vec();
        let verified = match self.containers.verified_host_pid(&summary.key, platform) {
            Ok(pid) => pid,
            Err(error) => {
                issues.push(DiagnosticIssue::new(
                    DiagnosticCode::ExternalToolFailed,
                    error.to_string(),
                ));
                None
            }
        };
        container_target(summary, verified, inventory, issues)
    }
}

fn container_target(
    summary: ContainerSummary,
    verified: Option<Pid>,
    inventory: &[runquiry_core::ProcessSummary],
    issues: Vec<DiagnosticIssue>,
) -> InvestigationTarget {
    if let Some(identity) = verified.and_then(|pid| PlatformBackend::identity_for(inventory, pid)) {
        return InvestigationTarget::Process(identity);
    }
    InvestigationTarget::Container(ContainerInvestigation {
        summary,
        verified_host_pid: verified,
        issues: issues.into(),
    })
}

fn published_container_resolution(
    candidates: Vec<InvestigationTarget>,
    subject: String,
) -> Result<Resolution<InvestigationTarget>, InspectError> {
    match candidates.as_slice() {
        [] => Err(InspectError::SocketOwnerUnknown { subject }),
        [candidate] => Ok(Resolution::Unique(candidate.clone())),
        _ => Ok(Resolution::Ambiguous(candidates)),
    }
}

#[cfg(test)]
mod tests {
    use runquiry_core::{ContainerKey, ContainerSummary, DiagnosticCode, DiagnosticIssue};
    use runquiry_ui::backend::InvestigationTarget;

    use super::{container_target, published_container_resolution};

    #[test]
    fn unique_container_without_verified_host_pid_remains_a_container_result() {
        let summary = ContainerSummary {
            key: ContainerKey {
                runtime: String::from("docker"),
                id: String::from("abc123"),
            },
            name: Some(String::from("web")),
            image: Some(String::from("example/web:1")),
            status: Some(String::from("running")),
            health: Some(String::from("healthy")),
            host_pid: None,
            started_at: None,
        };

        let target = container_target(summary, None, &[], Vec::new());

        assert!(matches!(&target, InvestigationTarget::Container(_)));
        if let InvestigationTarget::Container(container) = target {
            assert_eq!(container.summary.name.as_deref(), Some("web"));
            assert!(container.verified_host_pid.is_none());
        }
    }

    #[test]
    fn published_port_without_verified_host_pid_resolves_to_container_fallback()
    -> Result<(), runquiry_core::InspectError> {
        let summary = ContainerSummary {
            key: ContainerKey {
                runtime: String::from("docker"),
                id: String::from("published-abc123"),
            },
            name: Some(String::from("published-web")),
            image: Some(String::from("example/web:1")),
            status: Some(String::from("running")),
            health: None,
            host_pid: None,
            started_at: None,
        };
        let target = container_target(
            summary,
            None,
            &[],
            vec![DiagnosticIssue::new(
                DiagnosticCode::ExternalToolFailed,
                String::from("docker inspect 未提供可验证的宿主 PID"),
            )],
        );

        let resolution = published_container_resolution(
            vec![target],
            String::from("端口 8080 的 socket 属主不可知"),
        )?;

        let runquiry_core::Resolution::Unique(target) = resolution else {
            return Err(runquiry_core::InspectError::Unsupported {
                reason: String::from("已发布容器缺少可验证宿主 PID 时必须唯一回退到容器详情"),
            });
        };
        let InvestigationTarget::Container(container) = target else {
            return Err(runquiry_core::InspectError::Unsupported {
                reason: String::from("已发布容器缺少可验证宿主 PID 时不得进入进程分析"),
            });
        };
        assert_eq!(container.summary.name.as_deref(), Some("published-web"));
        assert!(container.verified_host_pid.is_none());
        assert_eq!(container.issues.len(), 1);
        assert_eq!(
            container.issues[0].code(),
            DiagnosticCode::ExternalToolFailed
        );
        Ok(())
    }
}
