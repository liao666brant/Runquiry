//! 平台只读端口到 UI 后端边界的装配。

mod analysis_gate;
mod container;
mod unavailable;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::time::SystemTime;

use runquiry_core::{
    Analysis, AnalysisPorts, ContainerInventory, FileInventory, InspectError, Inspection,
    NetworkInventory, Pid, ProcessIdentity, ProcessInventory, ProcessSummary, QueryTarget,
    Resolution, analyze, resolve_file_holders, resolve_name, resolve_port_owner,
};
use runquiry_platform::{container::ContainerRuntimes, linux::LinuxPlatform};
use runquiry_ui::WorkspaceId;
use runquiry_ui::backend::{
    ContainerSnapshotRow, InvestigationTarget, PortSnapshotRow, WorkspaceBackend, WorkspaceSnapshot,
};

pub use self::unavailable::UnavailableBackend;

/// Linux 平台的共享只读后端。
#[derive(Debug)]
pub struct PlatformBackend {
    analysis_gate: analysis_gate::AnalysisGate,
    containers: Arc<ContainerRuntimes>,
}

impl PlatformBackend {
    /// 构造共享的容器运行时与分析门控；Linux 平台按操作重建。
    pub(super) fn new() -> std::io::Result<Self> {
        let _ = LinuxPlatform::new()?;
        let containers = Arc::new(ContainerRuntimes::new());
        Ok(Self {
            analysis_gate: analysis_gate::AnalysisGate::new(),
            containers,
        })
    }

    fn fresh_platform() -> Result<LinuxPlatform, InspectError> {
        LinuxPlatform::new().map_err(|error| InspectError::Unsupported {
            reason: format!("Linux 平台采集器不可用：{error}"),
        })
    }

    fn identities(platform: &LinuxPlatform) -> Result<Vec<ProcessSummary>, InspectError> {
        ProcessInventory::list(platform)
            .data
            .ok_or_else(|| InspectError::Unsupported {
                reason: String::from("进程清单采集未返回数据"),
            })
    }

    fn identity_for(inventory: &[ProcessSummary], pid: Pid) -> Option<ProcessIdentity> {
        inventory
            .iter()
            .find(|process| process.identity.pid() == pid)
            .map(|process| process.identity.clone())
    }

    fn map_pids(
        inventory: &[ProcessSummary],
        resolution: Resolution<Pid>,
    ) -> Result<Resolution<ProcessIdentity>, InspectError> {
        let identities: Vec<_> = resolution
            .into_candidates()
            .into_iter()
            .filter_map(|pid| Self::identity_for(inventory, pid))
            .collect();
        match identities.as_slice() {
            [] => Err(InspectError::NotFound {
                subject: String::from("仍存活的进程候选"),
            }),
            [identity] => Ok(Resolution::Unique(identity.clone())),
            _ => Ok(Resolution::Ambiguous(identities)),
        }
    }
}

impl WorkspaceBackend for PlatformBackend {
    fn load(&self, workspace: WorkspaceId) -> WorkspaceSnapshot {
        let platform = match LinuxPlatform::new() {
            Ok(platform) => platform,
            Err(error) => return UnavailableBackend::new(error.to_string()).load(workspace),
        };
        match workspace {
            WorkspaceId::Processes => WorkspaceSnapshot::Processes {
                capability: ProcessInventory::capability(&platform),
                inspection: ProcessInventory::list(&platform).map(Arc::from),
            },
            WorkspaceId::Ports => {
                let inventory = ProcessInventory::list(&platform);
                WorkspaceSnapshot::Ports {
                    capability: NetworkInventory::capability(&platform),
                    inspection: platform.open_ports().map(|ports| {
                        ports
                            .into_iter()
                            .map(|entry| PortSnapshotRow {
                                process: entry.pid.and_then(|pid| {
                                    inventory.data.as_ref().and_then(|items| {
                                        items
                                            .iter()
                                            .find(|item| item.identity.pid() == pid)
                                            .map(|item| item.command.clone())
                                    })
                                }),
                                entry,
                            })
                            .collect::<Vec<_>>()
                            .into()
                    }),
                }
            }
            WorkspaceId::Containers => {
                let inspection = ContainerInventory::list(self.containers.as_ref()).map(|items| {
                    items
                        .into_iter()
                        .map(|summary| {
                            let verified_host_pid = self
                                .containers
                                .verified_host_pid(&summary.key, &platform)
                                .ok()
                                .flatten();
                            ContainerSnapshotRow {
                                summary,
                                verified_host_pid,
                            }
                        })
                        .collect::<Vec<_>>()
                        .into()
                });
                WorkspaceSnapshot::Containers {
                    capability: ContainerInventory::capability(self.containers.as_ref()),
                    inspection,
                }
            }
            WorkspaceId::FileLocks => WorkspaceSnapshot::FileLocks {
                capability: FileInventory::capability(&platform),
                inspection: FileInventory::list(&platform).map(Arc::from),
            },
        }
    }

    fn resolve(
        &self,
        target: &QueryTarget,
    ) -> Result<Resolution<InvestigationTarget>, InspectError> {
        let platform = Self::fresh_platform()?;
        if let QueryTarget::Container { query, exact } = target {
            return self.resolve_container(query, *exact, &platform);
        }
        let inventory = Self::identities(&platform)?;
        let pids = match target {
            QueryTarget::Pid(pid) => Resolution::Unique(*pid),
            QueryTarget::ProcessName { query, exact } => {
                resolve_name(&inventory, query, *exact, &[], None)?
            }
            QueryTarget::Port(port) => {
                let ports =
                    platform
                        .open_ports()
                        .data
                        .ok_or_else(|| InspectError::Unsupported {
                            reason: format!("端口 {port} 的采集未返回数据"),
                        })?;
                match resolve_port_owner(&ports, *port) {
                    Ok(resolution) => resolution,
                    Err(InspectError::SocketOwnerUnknown { subject }) => {
                        return self.resolve_published_port_container(
                            *port, subject, &inventory, &platform,
                        );
                    }
                    Err(error) => return Err(error),
                }
            }
            QueryTarget::File(path) => {
                let holders = FileInventory::holders(&platform, path)
                    .data
                    .ok_or_else(|| InspectError::Unsupported {
                        reason: format!("文件 {} 的采集未返回数据", path.display()),
                    })?;
                resolve_file_holders(&holders, path)?
            }
            QueryTarget::Container { query, exact } => {
                return self.resolve_container(query, *exact, &platform);
            }
        };
        Self::map_pids(&inventory, pids).map(|resolution| match resolution {
            Resolution::Unique(identity) => {
                Resolution::Unique(InvestigationTarget::Process(identity))
            }
            Resolution::Ambiguous(identities) => Resolution::Ambiguous(
                identities
                    .into_iter()
                    .map(InvestigationTarget::Process)
                    .collect(),
            ),
        })
    }

    fn analyze(&self, identity: &ProcessIdentity) -> Result<Inspection<Analysis>, InspectError> {
        self.with_analysis_gate(|| {
            let platform = Self::fresh_platform()?;
            let ports = AnalysisPorts {
                inventory: &platform,
                details: &platform,
                network: &platform,
                containers: self.containers.as_ref(),
                evidence: &platform,
                healthcheck: Some(self.containers.as_ref()),
                process_locks: Some(&platform),
            };
            analyze(identity, &ports, SystemTime::now(), false)
        })
    }
}
