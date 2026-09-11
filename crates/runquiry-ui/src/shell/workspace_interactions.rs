//! 工作区筛选、模式、排序与稳定选择的统一 generation 接线。

use gpui_kit::{Context, Window};

use super::AppShell;
use crate::session::{SortOrder, WorkspaceId};
use crate::workspaces::{
    FileKey, FileLockMode, PortKey, PortMode, update_containers_table, update_file_locks_table,
    update_ports_table,
};

impl AppShell {
    pub(crate) fn apply_filter(&mut self, value: String, cx: &mut Context<'_, Self>) {
        self.invalidate_active_refresh();
        let filter = (!value.is_empty()).then(|| value.clone());
        self.session.active_session_mut().set_filter(filter);
        match self.active_workspace() {
            WorkspaceId::Processes => self.data.filter_processes(value, cx),
            WorkspaceId::Ports => {
                self.data.ports.set_filter(value);
                update_ports_table(&self.data.ports_table, self.data.ports.clone(), cx);
            }
            WorkspaceId::Containers => {
                self.data.containers.set_filter(value);
                update_containers_table(
                    &self.data.containers_table,
                    self.data.containers.clone(),
                    cx,
                );
            }
            WorkspaceId::FileLocks => {
                self.data.files.set_filter(value);
                update_file_locks_table(&self.data.files_table, self.data.files.clone(), cx);
            }
        }
        cx.notify();
    }

    pub(crate) fn toggle_process_pid_sort(&mut self, cx: &mut Context<'_, Self>) {
        self.invalidate_active_refresh();
        self.record_sort(String::from("pid"));
        self.data.toggle_process_pid_sort(cx);
        cx.notify();
    }

    pub(crate) fn reveal_sensitive(&mut self, cx: &mut Context<'_, Self>) {
        self.data.processes.privacy_mut().reveal();
        cx.notify();
    }

    pub(crate) fn set_port_mode(&mut self, mode: PortMode, cx: &mut Context<'_, Self>) {
        self.invalidate_active_refresh();
        self.session.active_session_mut().invalidate_context();
        self.data.ports.set_mode(mode);
        update_ports_table(&self.data.ports_table, self.data.ports.clone(), cx);
        cx.notify();
    }

    pub(crate) fn set_file_mode(&mut self, mode: FileLockMode, cx: &mut Context<'_, Self>) {
        self.invalidate_active_refresh();
        self.session.active_session_mut().invalidate_context();
        self.data.files.set_mode(mode);
        update_file_locks_table(&self.data.files_table, self.data.files.clone(), cx);
        cx.notify();
    }

    pub(crate) fn select_port(
        &mut self,
        key: Option<PortKey>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.invalidate_active_refresh();
        let stable = key.as_ref().map(|key| {
            format!(
                "{:?}:{}:{}:{}:{:?}",
                key.protocol, key.address, key.port, key.state, key.pid
            )
        });
        self.session.active_session_mut().select(stable);
        self.data.ports.selection.select(key);
        Self::open_compact_detail(window, cx);
        cx.notify();
    }

    pub(crate) fn select_container(
        &mut self,
        key: Option<runquiry_core::ContainerKey>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.invalidate_active_refresh();
        let stable = key.as_ref().map(runquiry_core::ContainerKey::dedup_key);
        self.session.active_session_mut().select(stable);
        self.data.containers.selection.select(key);
        Self::open_compact_detail(window, cx);
        cx.notify();
    }

    pub(crate) fn select_file(
        &mut self,
        key: Option<FileKey>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.invalidate_active_refresh();
        let stable = key
            .as_ref()
            .map(|key| format!("{}:{}", key.pid, key.path.display()));
        self.session.active_session_mut().select(stable);
        self.data.files.selection.select(key);
        Self::open_compact_detail(window, cx);
        cx.notify();
    }

    pub(crate) fn record_table_sort(&mut self, column: usize) {
        self.invalidate_active_refresh();
        self.record_sort(format!("column-{column}"));
    }

    fn record_sort(&mut self, column: String) {
        let current = self.session.active_session().sort.as_ref();
        let ascending = current.is_none_or(|sort| sort.column != column || !sort.ascending);
        self.session
            .active_session_mut()
            .set_sort(Some(SortOrder { column, ascending }));
    }

    pub(crate) fn invalidate_active_refresh(&mut self) {
        let Some((gate, _)) = self.refresh_started.take() else {
            return;
        };
        self.session
            .session_mut(gate.workspace())
            .abort_refresh(gate.generation());
    }
}
