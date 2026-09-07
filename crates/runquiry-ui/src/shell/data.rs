//! 四个工作区的可视状态与 `DataTable` 实体。

use std::sync::Arc;

use gpui::{App, AppContext as _, Entity, Window};
use gpui_component::table::TableState;
use runquiry_core::{CapabilityStatus, DiagnosticIssue};

use crate::backend::WorkspaceSnapshot;
use crate::processes::{ProcessTableDelegate, ProcessesState, SurfaceState};
use crate::workspaces::{
    ContainerRow, ContainersState, ContainersTableDelegate, FileLocksState, FileLocksTableDelegate,
    PortRow, PortsState, PortsTableDelegate, update_containers_table, update_file_locks_table,
    update_ports_table,
};

/// 壳层持有的四页数据。
pub(super) struct ShellData {
    pub processes: ProcessesState,
    pub ports: PortsState,
    pub containers: ContainersState,
    pub files: FileLocksState,
    pub process_capability: CapabilityStatus,
    pub process_issues: Arc<[DiagnosticIssue]>,
    process_all: Arc<[runquiry_core::ProcessSummary]>,
    process_filter: String,
    process_descending: bool,
    pub process_table: Entity<TableState<ProcessTableDelegate>>,
    pub ports_table: Entity<TableState<PortsTableDelegate>>,
    pub containers_table: Entity<TableState<ContainersTableDelegate>>,
    pub files_table: Entity<TableState<FileLocksTableDelegate>>,
}

impl ShellData {
    pub(super) fn new(window: &mut Window, cx: &mut App) -> Self {
        let processes = ProcessesState::new(Arc::default());
        let ports = PortsState::default();
        let containers = ContainersState::default();
        let files = FileLocksState::default();
        let process_table = cx.new(|cx| {
            TableState::new(ProcessTableDelegate::new(Arc::default()), window, cx)
                .row_selectable(true)
                .col_selectable(false)
                .cell_selectable(false)
                .loop_selection(false)
        });
        let ports_table = crate::workspaces::new_ports_table(ports.clone(), window, cx);
        let containers_table =
            crate::workspaces::new_containers_table(containers.clone(), window, cx);
        let files_table = crate::workspaces::new_file_locks_table(files.clone(), window, cx);
        Self {
            processes,
            ports,
            containers,
            files,
            process_capability: CapabilityStatus::Supported,
            process_issues: Arc::default(),
            process_all: Arc::default(),
            process_filter: String::new(),
            process_descending: false,
            process_table,
            ports_table,
            containers_table,
            files_table,
        }
    }

    pub(super) fn apply(&mut self, snapshot: WorkspaceSnapshot, cx: &mut App) {
        match snapshot {
            WorkspaceSnapshot::Processes {
                capability,
                inspection,
            } => {
                let issues: Arc<[DiagnosticIssue]> = inspection.issues.into();
                let rows = inspection.data.unwrap_or_default();
                self.process_all = rows;
                let rows = self.filtered_processes();
                self.processes.replace_rows(Arc::clone(&rows));
                self.process_capability = capability.clone();
                self.process_issues = issues;
                self.process_table.update(cx, |table, cx| {
                    table.delegate_mut().replace_rows(rows);
                    table
                        .delegate_mut()
                        .set_surface(process_surface(&capability, &self.process_issues));
                    table.refresh(cx);
                });
            }
            WorkspaceSnapshot::Ports {
                capability,
                inspection,
            } => {
                let rows = inspection.map(|entries| {
                    entries
                        .iter()
                        .cloned()
                        .map(|row| PortRow::new(row.entry, row.process))
                        .collect::<Vec<_>>()
                        .into()
                });
                self.ports
                    .apply(self.ports.load.generation, &capability, rows);
                update_ports_table(&self.ports_table, self.ports.clone(), cx);
            }
            WorkspaceSnapshot::Containers {
                capability,
                inspection,
            } => {
                let rows = inspection.map(|entries| {
                    entries
                        .iter()
                        .cloned()
                        .map(|row| ContainerRow::new(row.summary, row.verified_host_pid))
                        .collect::<Vec<_>>()
                        .into()
                });
                self.containers
                    .apply(self.containers.load.generation, &capability, rows);
                update_containers_table(&self.containers_table, self.containers.clone(), cx);
            }
            WorkspaceSnapshot::FileLocks {
                capability,
                inspection,
            } => {
                self.files
                    .apply(self.files.load.generation, &capability, inspection);
                update_file_locks_table(&self.files_table, self.files.clone(), cx);
            }
        }
    }

    pub(super) fn relocalize(&self, cx: &mut App) {
        self.process_table.update(cx, |table, cx| {
            table.delegate_mut().relocalize();
            table.refresh(cx);
        });
        self.ports_table.update(cx, |table, cx| {
            table.delegate_mut().relocalize();
            table.refresh(cx);
        });
        self.containers_table.update(cx, |table, cx| {
            table.delegate_mut().relocalize();
            table.refresh(cx);
        });
        self.files_table.update(cx, |table, cx| {
            table.delegate_mut().relocalize();
            table.refresh(cx);
        });
    }

    pub(super) fn filter_processes(&mut self, value: String, cx: &mut App) {
        self.process_filter = value;
        self.refresh_process_table(cx);
    }

    pub(super) fn toggle_process_pid_sort(&mut self, cx: &mut App) {
        self.process_descending = !self.process_descending;
        self.refresh_process_table(cx);
    }

    fn refresh_process_table(&mut self, cx: &mut App) {
        let rows = self.filtered_processes();
        self.processes.replace_rows(Arc::clone(&rows));
        self.process_table.update(cx, |table, cx| {
            table.delegate_mut().replace_rows(rows);
            table.refresh(cx);
        });
    }

    fn filtered_processes(&self) -> Arc<[runquiry_core::ProcessSummary]> {
        let needle = self.process_filter.to_lowercase();
        let mut rows: Vec<_> = self
            .process_all
            .iter()
            .filter(|row| {
                needle.is_empty()
                    || row.command.to_lowercase().contains(&needle)
                    || row.identity.pid().to_string().contains(&needle)
                    || row
                        .user
                        .as_ref()
                        .is_some_and(|user| user.to_lowercase().contains(&needle))
            })
            .cloned()
            .collect();
        rows.sort_by_key(|row| row.identity.pid());
        if self.process_descending {
            rows.reverse();
        }
        rows.into()
    }
}

fn process_surface(capability: &CapabilityStatus, issues: &[DiagnosticIssue]) -> SurfaceState {
    match capability {
        CapabilityStatus::Unsupported(reason) => SurfaceState::Unsupported {
            reason: reason.clone(),
        },
        CapabilityStatus::Unavailable(reason) => SurfaceState::Unavailable {
            reason: reason.clone(),
        },
        CapabilityStatus::Partial(_) if !issues.is_empty() => SurfaceState::Partial {
            issue_count: issues.len(),
        },
        CapabilityStatus::Supported | CapabilityStatus::Partial(_) if !issues.is_empty() => {
            SurfaceState::Partial {
                issue_count: issues.len(),
            }
        }
        CapabilityStatus::Supported | CapabilityStatus::Partial(_) => SurfaceState::Ready,
    }
}
