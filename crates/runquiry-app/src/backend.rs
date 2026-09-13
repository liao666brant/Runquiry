//! 平台只读端口到 UI 后端边界的装配。

mod analysis_gate;
mod container;
mod cpu_sample;
mod unavailable;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::time::SystemTime;

use runquiry_core::{
    Analysis, AnalysisPorts, CapabilityStatus, ContainerInventory, DiagnosticCode, FileInventory,
    InspectError, Inspection, NetworkInventory, Pid, ProcessAction, ProcessController,
    ProcessIdentity, ProcessInventory, ProcessSummary, QueryTarget, Renice, Resolution, analyze,
    resolve_file_holders, resolve_name, resolve_port_owner,
};
use runquiry_ui::WorkspaceId;
use runquiry_ui::backend::{
    ContainerSnapshotRow, InvestigationTarget, PortSnapshotRow, ProcessActionCapabilities,
    WorkspaceBackend, WorkspaceSnapshot,
};

// 目标平台选择（Batch 6 最小装配）：app 边界按 target_os 绑定单一平台结构体，
// 各平台结构体实现同一组 core 端口且构造签名一致（`new() -> io::Result<Self>`）。
// container 运行时为跨平台模块，不随平台 cfg。macOS 已移出 v1 范围。
use runquiry_platform::container::ContainerRuntimes;
#[cfg(target_os = "linux")]
use runquiry_platform::linux::LinuxPlatform as Platform;
#[cfg(target_os = "windows")]
use runquiry_platform::windows::WindowsPlatform as Platform;

pub use self::unavailable::UnavailableBackend;

/// 目标平台（按 target_os 选择 Linux/Windows）的共享只读后端。
#[derive(Debug)]
pub struct PlatformBackend {
    analysis_gate: analysis_gate::AnalysisGate,
    containers: Arc<ContainerRuntimes>,
    /// 进程 CPU% 两样本差分状态（跨刷新保留，见 [`cpu_sample`]）。
    cpu_samples: std::sync::Mutex<cpu_sample::CpuSampleStore>,
}

impl PlatformBackend {
    /// 构造共享的容器运行时与分析门控；目标平台按操作重建。
    pub(super) fn new() -> std::io::Result<Self> {
        let _ = Platform::new()?;
        let containers = Arc::new(ContainerRuntimes::new());
        Ok(Self {
            analysis_gate: analysis_gate::AnalysisGate::new(),
            containers,
            cpu_samples: std::sync::Mutex::new(cpu_sample::CpuSampleStore::default()),
        })
    }

    fn fresh_platform() -> Result<Platform, InspectError> {
        Platform::new().map_err(|error| InspectError::Unsupported {
            reason: format!("平台采集器不可用：{error}"),
        })
    }

    /// 把「采集未返回数据」的失败转换为结构化错误：平台标注的能力不支持
    /// 原样保留为 `Unsupported`（如 Windows 文件锁），权限失败保留 subject，
    /// 其余保留首条诊断，不把所有失败折叠成同一文案。
    fn failed_collection_error<T>(
        inspection: &Inspection<T>,
        subject: String,
        fallback: String,
    ) -> InspectError {
        if let Some(issue) = inspection
            .issues
            .iter()
            .find(|issue| issue.code() == DiagnosticCode::Unsupported)
        {
            return InspectError::Unsupported {
                reason: issue.message().to_owned(),
            };
        }
        if inspection
            .issues
            .iter()
            .any(|issue| issue.code() == DiagnosticCode::PermissionDenied)
        {
            return InspectError::PermissionDenied { subject };
        }
        InspectError::Unsupported {
            reason: match inspection.issues.first() {
                Some(issue) => format!("{fallback}：{}", issue.message()),
                None => fallback,
            },
        }
    }

    fn identities(platform: &Platform) -> Result<Vec<ProcessSummary>, InspectError> {
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
        let platform = match Platform::new() {
            Ok(platform) => platform,
            Err(error) => return UnavailableBackend::new(error.to_string()).load(workspace),
        };
        match workspace {
            WorkspaceId::Processes => {
                let mut inspection = ProcessInventory::list(&platform);
                // 两样本差分：写回本轮 cpu_percent（锁中毒按数据原样恢复，
                // 采样状态丢失只影响一次差分）。
                if let Some(summaries) = inspection.data.as_mut() {
                    self.cpu_samples
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .apply(summaries);
                }
                WorkspaceSnapshot::Processes {
                    capability: ProcessInventory::capability(&platform),
                    inspection: inspection.map(Arc::from),
                }
            }
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
            // 名称解析零命中后的平台服务回退（launchd / systemd / SCM）已随
            // macOS 移出 v1 范围而移除；Linux/Windows 当前无服务 PID 回退。
            QueryTarget::ProcessName { query, exact } => {
                resolve_name(&inventory, query, *exact, &[], None)?
            }
            QueryTarget::Port(port) => {
                let ports_inspection = platform.open_ports();
                let ports = match ports_inspection.data {
                    Some(ports) => ports,
                    None => {
                        return Err(Self::failed_collection_error(
                            &ports_inspection,
                            format!("端口 {port}"),
                            format!("端口 {port} 的采集未返回数据"),
                        ));
                    }
                };
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
                let holders_inspection = FileInventory::holders(&platform, path);
                let holders = match holders_inspection.data {
                    Some(holders) => holders,
                    None => {
                        return Err(Self::failed_collection_error(
                            &holders_inspection,
                            format!("文件 {}", path.display()),
                            format!("文件 {} 的采集未返回数据", path.display()),
                        ));
                    }
                };
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

    fn process_control_capability(&self) -> CapabilityStatus {
        match Self::fresh_platform() {
            Ok(platform) => ProcessController::capability(&platform),
            // 平台结构构造失败是环境不可用而非平台不支持：保留能力四态区分。
            Err(InspectError::Unsupported { reason }) => CapabilityStatus::Unavailable(reason),
            Err(error) => CapabilityStatus::Unavailable(error.to_string()),
        }
    }

    fn process_action_capabilities(&self) -> ProcessActionCapabilities {
        match Self::fresh_platform() {
            Ok(platform) => {
                let action = |action: ProcessAction| {
                    ProcessController::action_capability(&platform, &action)
                };
                // Renice 载荷不影响能力结论（平台对整个 Renice 类别给出结
                // 论）；0 恒在合法区间，unwrap_or 分支不可达。
                let renice = Renice::try_from(0)
                    .map(ProcessAction::Renice)
                    .unwrap_or(ProcessAction::Terminate);
                ProcessActionCapabilities {
                    class: ProcessController::capability(&platform),
                    terminate: action(ProcessAction::Terminate),
                    kill: action(ProcessAction::Kill),
                    kill_tree: action(ProcessAction::KillTree),
                    pause: action(ProcessAction::Pause),
                    resume: action(ProcessAction::Resume),
                    renice: action(renice),
                    reveal: ProcessController::reveal_capability(&platform),
                }
            }
            // 平台结构构造失败是环境不可用而非平台不支持：保留能力四态区分。
            Err(error) => ProcessActionCapabilities::all_unavailable(error.to_string()),
        }
    }

    fn reveal_process_executable(&self, identity: &ProcessIdentity) -> Result<(), InspectError> {
        let platform = Self::fresh_platform()?;
        ProcessController::reveal_executable(&platform, identity)
    }

    fn execute_process_action(
        &self,
        identity: &ProcessIdentity,
        action: ProcessAction,
    ) -> Result<(), InspectError> {
        let platform = Self::fresh_platform()?;
        ProcessController::execute(&platform, identity, action)
    }
}
