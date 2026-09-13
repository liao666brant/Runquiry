//! 四个工作区的可视状态与 `DataTable` 实体。

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use gpui_kit::component::table::TableState;
use gpui_kit::{App, AppContext as _, Entity, Window};
use runquiry_core::{CapabilityStatus, DiagnosticCode, DiagnosticIssue, ProcessSummary};

use crate::backend::WorkspaceSnapshot;
use crate::processes::{
    ProcessSort, ProcessSortKey, ProcessTableDelegate, ProcessesState, SurfaceState,
};
use crate::session::WorkspaceId;
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
    process_sort: ProcessSort,
    /// 各工作区隐藏的表格列（工作区键 → 列 key 集合），启动时来自设置。
    hidden_columns: BTreeMap<String, BTreeSet<String>>,
    pub process_table: Entity<TableState<ProcessTableDelegate>>,
    pub ports_table: Entity<TableState<PortsTableDelegate>>,
    pub containers_table: Entity<TableState<ContainersTableDelegate>>,
    pub files_table: Entity<TableState<FileLocksTableDelegate>>,
}

impl ShellData {
    /// 当前工作区是否允许模式切换、筛选等交互；能力/环境边界上禁用，
    /// 不能制造「页面仍可操作」的错觉。
    pub(super) fn interactions_enabled(&self, workspace: WorkspaceId) -> bool {
        match workspace {
            // 与 SurfaceState::from_parts / map_state 的边界判定同源：
            // 能力边界或 Unsupported 诊断都禁用交互。
            WorkspaceId::Processes => {
                let capability_boundary = matches!(
                    self.process_capability,
                    CapabilityStatus::Unsupported(_) | CapabilityStatus::Unavailable(_)
                );
                let issue_boundary = self
                    .process_issues
                    .iter()
                    .any(|issue| issue.code() == DiagnosticCode::Unsupported);
                !(capability_boundary || issue_boundary)
            }
            WorkspaceId::Ports => crate::workspaces::interactions_enabled(self.ports.load.state),
            WorkspaceId::Containers => {
                crate::workspaces::interactions_enabled(self.containers.load.state)
            }
            WorkspaceId::FileLocks => {
                crate::workspaces::interactions_enabled(self.files.load.state)
            }
        }
    }

    pub(super) fn new(
        window: &mut Window,
        cx: &mut App,
        hidden_columns: BTreeMap<String, BTreeSet<String>>,
        shell: gpui_kit::WeakEntity<super::AppShell>,
    ) -> Self {
        let processes = ProcessesState::new(Arc::default());
        let ports = PortsState::default();
        let containers = ContainersState::default();
        let files = FileLocksState::default();
        let empty = BTreeSet::new();
        let hidden_of = |workspace: WorkspaceId| {
            hidden_columns
                .get(workspace.key())
                .unwrap_or(&empty)
                .clone()
        };
        let process_table = cx.new(|cx| {
            TableState::new(
                ProcessTableDelegate::new(Arc::default(), ProcessSort::default()),
                window,
                cx,
            )
            .row_selectable(true)
            .col_selectable(false)
            .cell_selectable(false)
            .loop_selection(false)
        });
        let ports_table = crate::workspaces::new_ports_table(ports.clone(), window, cx);
        let containers_table =
            crate::workspaces::new_containers_table(containers.clone(), window, cx);
        let files_table = crate::workspaces::new_file_locks_table(files.clone(), window, cx);
        // 启动时恢复持久化的列显隐（delegate 初始为全量可见），并注入壳层
        // 句柄供进程表右键菜单回发动作请求。
        process_table.update(cx, |table, _| {
            let delegate = table.delegate_mut();
            delegate.set_hidden(hidden_of(WorkspaceId::Processes));
            delegate.set_shell(shell.clone());
        });
        ports_table.update(cx, |table, _| {
            table
                .delegate_mut()
                .set_hidden(hidden_of(WorkspaceId::Ports));
        });
        containers_table.update(cx, |table, _| {
            table
                .delegate_mut()
                .set_hidden(hidden_of(WorkspaceId::Containers));
        });
        files_table.update(cx, |table, _| {
            table
                .delegate_mut()
                .set_hidden(hidden_of(WorkspaceId::FileLocks));
        });
        Self {
            processes,
            ports,
            containers,
            files,
            process_capability: CapabilityStatus::Supported,
            process_issues: Arc::default(),
            process_all: Arc::default(),
            process_filter: String::new(),
            process_sort: ProcessSort::default(),
            hidden_columns,
            process_table,
            ports_table,
            containers_table,
            files_table,
        }
    }

    /// 当前工作区隐藏的列 key 集合（只读视图的克隆）。
    pub(super) fn hidden_columns(&self, workspace: WorkspaceId) -> BTreeSet<String> {
        self.hidden_columns
            .get(workspace.key())
            .cloned()
            .unwrap_or_default()
    }

    /// 同步进程表右键菜单的动作可用快照（随 Processes 刷新的逐动作能力）。
    pub(super) fn set_kill_actions_available(&mut self, kill: bool, kill_tree: bool, cx: &mut App) {
        self.process_table.update(cx, |table, _| {
            table.delegate_mut().set_kill_actions(kill, kill_tree);
        });
    }

    /// 切换单列显隐：更新持久化状态源并同步到对应表格 delegate。
    pub(super) fn set_column_hidden(
        &mut self,
        workspace: WorkspaceId,
        key: &str,
        visible: bool,
        cx: &mut App,
    ) {
        let keys = self
            .hidden_columns
            .entry(String::from(workspace.key()))
            .or_default();
        if visible {
            keys.remove(key);
        } else {
            keys.insert(String::from(key));
        }
        let hidden = keys.clone();
        match workspace {
            WorkspaceId::Processes => self.process_table.update(cx, |table, cx| {
                table.delegate_mut().set_hidden(hidden);
                table.refresh(cx);
            }),
            WorkspaceId::Ports => self.ports_table.update(cx, |table, cx| {
                table.delegate_mut().set_hidden(hidden);
                table.refresh(cx);
            }),
            WorkspaceId::Containers => {
                self.containers_table.update(cx, |table, cx| {
                    table.delegate_mut().set_hidden(hidden);
                    table.refresh(cx);
                });
            }
            WorkspaceId::FileLocks => {
                self.files_table.update(cx, |table, cx| {
                    table.delegate_mut().set_hidden(hidden);
                    table.refresh(cx);
                });
            }
        }
    }

    pub(super) fn apply(&mut self, snapshot: WorkspaceSnapshot, cx: &mut App) {
        match snapshot {
            WorkspaceSnapshot::Processes {
                capability,
                inspection,
            } => {
                let issues: Arc<[DiagnosticIssue]> = inspection.issues.into();
                let has_snapshot = inspection.data.is_some();
                let rows = inspection.data.unwrap_or_default();
                self.process_all = rows;
                let rows = self.filtered_processes();
                self.processes.replace_rows(Arc::clone(&rows));
                self.process_capability = capability.clone();
                self.process_issues = issues;
                self.process_table.update(cx, |table, cx| {
                    table.delegate_mut().replace_rows(rows);
                    table.delegate_mut().set_surface(SurfaceState::from_parts(
                        &capability,
                        has_snapshot,
                        &self.process_issues,
                    ));
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

    /// 应用表头点击产生的新排序并重排（delegate 已先行更新自己的副本）。
    pub(super) fn apply_process_sort(&mut self, sort: ProcessSort, cx: &mut App) {
        self.process_sort = sort;
        self.refresh_process_table(cx);
    }

    fn refresh_process_table(&mut self, cx: &mut App) {
        let rows = self.filtered_processes();
        let sort = self.process_sort;
        self.processes.replace_rows(Arc::clone(&rows));
        self.process_table.update(cx, |table, cx| {
            let delegate = table.delegate_mut();
            delegate.replace_rows(rows);
            delegate.set_sort(sort);
            table.refresh(cx);
        });
    }

    fn filtered_processes(&self) -> Arc<[ProcessSummary]> {
        let needle = self.process_filter.to_lowercase();
        let mut rows: Vec<_> = self
            .process_all
            .iter()
            .filter(|row| {
                needle.is_empty()
                    || row.command.to_lowercase().contains(&needle)
                    || row
                        .command_line
                        .as_ref()
                        .is_some_and(|line| line.to_lowercase().contains(&needle))
                    || row.identity.pid().to_string().contains(&needle)
                    || row
                        .user
                        .as_ref()
                        .is_some_and(|user| user.to_lowercase().contains(&needle))
            })
            .cloned()
            .collect();
        self.process_sort.apply(&mut rows);
        rows.into()
    }
}

impl ProcessSort {
    /// 排序语义：方向只作用于有值的键；`None`（未采样/不可得）无论方向恒排
    /// 最后（witr 降序时空值在尾部）；同值与 `None` 行按 PID 升序保持稳定，
    /// 不随方向反转。
    fn apply(self, rows: &mut [ProcessSummary]) {
        rows.sort_by(|a, b| {
            let pid_ord = a.identity.pid().cmp(&b.identity.pid());
            // 统一为「有值键」：PID 也映射为值（u32 可被 f64 精确表示），于是
            // 排序只有一条路径——方向作用于值、空值恒排尾部。
            #[allow(clippy::cast_precision_loss)]
            let key_of = |row: &ProcessSummary| match self.key {
                ProcessSortKey::Pid => Some(f64::from(row.identity.pid().get())),
                ProcessSortKey::Cpu => row.cpu_percent,
                ProcessSortKey::Memory => row.memory_rss_bytes.map(|bytes| bytes as f64),
            };
            match (key_of(a), key_of(b)) {
                (None, None) => pid_ord,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(x), Some(y)) => {
                    let ordering = x.partial_cmp(&y).unwrap_or(Ordering::Equal);
                    if self.descending {
                        ordering.reverse()
                    } else {
                        ordering
                    }
                    .then(pid_ord)
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{ProcessSort, ProcessSortKey};
    use runquiry_core::{HealthStatus, Pid, ProcessIdentity, ProcessSummary};

    fn summary(pid: u32, cpu_percent: Option<f64>, rss: Option<u64>) -> ProcessSummary {
        ProcessSummary {
            identity: ProcessIdentity::new(Pid::new(pid).unwrap_or(Pid::MIN), None, None),
            parent_pid: None,
            command: String::from("proc"),
            command_line: None,
            user: None,
            health: HealthStatus::Unknown,
            container: None,
            exe_deleted: false,
            capabilities: Vec::new(),
            cpu_time_seconds: None,
            cpu_percent,
            memory_rss_bytes: rss,
            memory_percent: None,
        }
    }

    #[test]
    fn default_sort_is_cpu_descending_with_ties_by_pid_and_none_last() {
        let mut rows = vec![
            summary(1, Some(5.0), None),
            summary(2, None, None),
            summary(3, Some(9.0), None),
            summary(4, Some(5.0), None),
        ];
        ProcessSort::default().apply(&mut rows);

        let pids: Vec<_> = rows.iter().map(|row| row.identity.pid().get()).collect();
        assert_eq!(pids, vec![3, 1, 4, 2]);
    }

    #[test]
    fn missing_rows_keep_pid_order_at_the_tail_even_descending() {
        let mut rows = vec![
            summary(30, None, None),
            summary(10, Some(1.0), None),
            summary(20, None, None),
        ];
        ProcessSort::default().apply(&mut rows);

        let pids: Vec<_> = rows.iter().map(|row| row.identity.pid().get()).collect();
        assert_eq!(pids, vec![10, 20, 30]);
    }

    #[test]
    fn memory_sort_uses_rss_bytes_ascending_when_requested() {
        let mut rows = vec![
            summary(1, None, Some(300)),
            summary(2, None, Some(100)),
            summary(3, None, None),
        ];
        ProcessSort {
            key: ProcessSortKey::Memory,
            descending: false,
        }
        .apply(&mut rows);

        let pids: Vec<_> = rows.iter().map(|row| row.identity.pid().get()).collect();
        assert_eq!(pids, vec![2, 1, 3]);
    }

    #[test]
    fn pid_sort_defaults_to_ascending() {
        let mut rows = vec![summary(30, None, None), summary(10, None, None)];
        ProcessSort {
            key: ProcessSortKey::Pid,
            descending: false,
        }
        .apply(&mut rows);

        let pids: Vec<_> = rows.iter().map(|row| row.identity.pid().get()).collect();
        assert_eq!(pids, vec![10, 30]);
    }
}
