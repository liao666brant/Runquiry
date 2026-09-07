use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::SystemTime,
};

use runquiry_core::{
    Analysis, CapabilityStatus, ContainerKey, ContainerSummary, FileInventoryEntry, HealthStatus,
    InspectError, Inspection, LockMetadata, LockMode, LockType, OpenPortEntry, Pid, Port,
    ProcessAction, ProcessIdentity, ProcessSummary, Protocol, QueryTarget, Resolution,
};
use runquiry_ui::{
    WorkspaceId,
    backend::{
        ContainerSnapshotRow, InvestigationTarget, PortSnapshotRow, WorkspaceBackend,
        WorkspaceSnapshot,
    },
};

const SCALE_ROWS: usize = 100_000;
struct ScaleSnapshots {
    processes: Arc<[ProcessSummary]>,
    ports: Arc<[PortSnapshotRow]>,
    containers: Arc<[ContainerSnapshotRow]>,
    file_locks: Arc<[FileInventoryEntry]>,
}

impl ScaleSnapshots {
    fn build() -> Result<Self, InspectError> {
        Ok(Self {
            processes: build_processes()?,
            ports: build_ports()?,
            containers: build_containers()?,
            file_locks: build_file_locks()?,
        })
    }
}

pub(crate) struct SyntheticBackend {
    snapshots: ScaleSnapshots,
    load_count: AtomicUsize,
}

impl SyntheticBackend {
    pub(crate) fn new() -> Result<Self, InspectError> {
        Ok(Self {
            snapshots: ScaleSnapshots::build()?,
            load_count: AtomicUsize::new(0),
        })
    }

    fn capability_for(&self) -> CapabilityStatus {
        let refresh = self
            .load_count
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        if refresh == 1 {
            CapabilityStatus::Supported
        } else {
            CapabilityStatus::Partial(format!("Synthetic scale QA refresh #{refresh}"))
        }
    }

    #[cfg(test)]
    fn completed_loads(&self) -> usize {
        self.load_count.load(Ordering::Relaxed)
    }
}

impl WorkspaceBackend for SyntheticBackend {
    fn load(&self, workspace: WorkspaceId) -> WorkspaceSnapshot {
        let capability = self.capability_for();
        match workspace {
            WorkspaceId::Processes => WorkspaceSnapshot::Processes {
                capability,
                inspection: Inspection::complete(Arc::clone(&self.snapshots.processes)),
            },
            WorkspaceId::Ports => WorkspaceSnapshot::Ports {
                capability,
                inspection: Inspection::complete(Arc::clone(&self.snapshots.ports)),
            },
            WorkspaceId::Containers => WorkspaceSnapshot::Containers {
                capability,
                inspection: Inspection::complete(Arc::clone(&self.snapshots.containers)),
            },
            WorkspaceId::FileLocks => WorkspaceSnapshot::FileLocks {
                capability,
                inspection: Inspection::complete(Arc::clone(&self.snapshots.file_locks)),
            },
        }
    }

    fn resolve(&self, _: &QueryTarget) -> Result<Resolution<InvestigationTarget>, InspectError> {
        Err(InspectError::Unsupported {
            reason: String::from("Synthetic scale QA does not provide investigations"),
        })
    }

    fn analyze(&self, _: &ProcessIdentity) -> Result<Inspection<Analysis>, InspectError> {
        Err(InspectError::Unsupported {
            reason: String::from("Synthetic scale QA does not provide analysis"),
        })
    }

    fn process_control_capability(&self) -> CapabilityStatus {
        CapabilityStatus::Unsupported(String::from("Synthetic scale QA never controls processes"))
    }

    fn execute_process_action(
        &self,
        _: &ProcessIdentity,
        _: ProcessAction,
    ) -> Result<(), InspectError> {
        Err(InspectError::Unsupported {
            reason: String::from("Synthetic scale QA never controls processes"),
        })
    }
}

fn build_processes() -> Result<Arc<[ProcessSummary]>, InspectError> {
    let mut rows = Vec::with_capacity(SCALE_ROWS);
    for index in 0..SCALE_ROWS {
        let pid = synthetic_pid(index)?;
        rows.push(ProcessSummary {
            identity: ProcessIdentity::new(pid, Some(SystemTime::UNIX_EPOCH), None),
            parent_pid: None,
            command: format!("synthetic-process-{index:06}"),
            command_line: None,
            user: Some(String::from("synthetic-qa")),
            health: HealthStatus::Healthy,
            container: None,
            exe_deleted: false,
            capabilities: Vec::new(),
        });
    }
    Ok(rows.into())
}

fn build_ports() -> Result<Arc<[PortSnapshotRow]>, InspectError> {
    let mut rows = Vec::with_capacity(SCALE_ROWS);
    for index in 0..SCALE_ROWS {
        let ordinal = synthetic_ordinal(index)?;
        rows.push(PortSnapshotRow {
            entry: OpenPortEntry {
                pid: Some(synthetic_pid(index)?),
                port: synthetic_port(ordinal)?,
                address: format!(
                    "10.{}.{}.{}",
                    ordinal / 65_536,
                    ordinal / 256 % 256,
                    ordinal % 256
                ),
                protocol: Protocol::Tcp,
                state: String::from("LISTEN"),
            },
            process: Some(format!("synthetic-process-{index:06}")),
        });
    }
    Ok(rows.into())
}

fn build_containers() -> Result<Arc<[ContainerSnapshotRow]>, InspectError> {
    let mut rows = Vec::with_capacity(SCALE_ROWS);
    for index in 0..SCALE_ROWS {
        rows.push(ContainerSnapshotRow {
            summary: ContainerSummary {
                key: ContainerKey {
                    runtime: String::from("synthetic"),
                    id: format!("scale-{index:06}"),
                },
                name: Some(format!("synthetic-container-{index:06}")),
                image: Some(String::from("synthetic/scale-qa:latest")),
                status: Some(String::from("running")),
                health: Some(String::from("healthy")),
                host_pid: Some(synthetic_pid(index)?),
                started_at: Some(SystemTime::UNIX_EPOCH),
            },
            verified_host_pid: Some(synthetic_pid(index)?),
        });
    }
    Ok(rows.into())
}

fn build_file_locks() -> Result<Arc<[FileInventoryEntry]>, InspectError> {
    let mut rows = Vec::with_capacity(SCALE_ROWS);
    for index in 0..SCALE_ROWS {
        rows.push(FileInventoryEntry {
            pid: synthetic_pid(index)?,
            process: format!("synthetic-process-{index:06}"),
            path: PathBuf::from(format!("/synthetic/scale-qa/file-{index:06}")),
            fd: None,
            lock: Some(LockMetadata {
                lock_type: LockType::Posix,
                mode: LockMode::Write,
            }),
        });
    }
    Ok(rows.into())
}

fn synthetic_ordinal(index: usize) -> Result<u32, InspectError> {
    u32::try_from(index).map_err(|error| InspectError::InvalidTarget {
        reason: error.to_string(),
    })
}

fn synthetic_pid(index: usize) -> Result<Pid, InspectError> {
    let ordinal = synthetic_ordinal(index)?;
    Pid::new(ordinal.saturating_add(1)).map_err(|error| InspectError::InvalidTarget {
        reason: error.to_string(),
    })
}

fn synthetic_port(ordinal: u32) -> Result<Port, InspectError> {
    let value =
        u16::try_from(ordinal % 65_535 + 1).map_err(|error| InspectError::InvalidTarget {
            reason: error.to_string(),
        })?;
    Port::new(value).map_err(|error| InspectError::InvalidTarget {
        reason: error.to_string(),
    })
}

#[cfg(test)]
#[path = "backend_tests.rs"]
mod tests;
