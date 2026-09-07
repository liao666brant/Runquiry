//! 按稳定领域键从当前工作区快照派生详情。

use std::sync::Arc;

use runquiry_core::{DiagnosticIssue, FileInventoryEntry};

use super::{ContainerRow, ContainersState, FileLocksState, PortRow, PortsState};

/// 当前选择对应的真实领域行；`Stale` 表示键已不在最新快照中。
#[derive(Clone, Debug)]
pub enum SelectedWorkspaceRow {
    /// 端口行。
    Port(PortRow),
    /// 容器行。
    Container(ContainerRow),
    /// 文件行。
    File(FileInventoryEntry),
    /// 选择已过期。
    Stale,
}

/// 工作区详情与同一次快照携带的诊断。
#[derive(Clone, Debug)]
pub struct SelectedWorkspaceDetail {
    /// 当前领域行或过期标记。
    pub row: SelectedWorkspaceRow,
    /// 原始 partial issues。
    pub issues: Arc<[DiagnosticIssue]>,
}

impl PortsState {
    /// 按稳定端口键从当前快照取得详情。
    pub fn selected_detail(&self) -> Option<SelectedWorkspaceDetail> {
        let key = self.selection.selected()?;
        let row = self
            .load
            .rows
            .iter()
            .find(|row| row.key() == *key)
            .cloned()
            .map_or(SelectedWorkspaceRow::Stale, SelectedWorkspaceRow::Port);
        Some(SelectedWorkspaceDetail {
            row,
            issues: Arc::clone(&self.load.issues),
        })
    }
}

impl ContainersState {
    /// 按稳定容器键从当前快照取得详情。
    pub fn selected_detail(&self) -> Option<SelectedWorkspaceDetail> {
        let key = self.selection.selected()?;
        let row = self
            .load
            .rows
            .iter()
            .find(|row| row.key() == key)
            .cloned()
            .map_or(SelectedWorkspaceRow::Stale, SelectedWorkspaceRow::Container);
        Some(SelectedWorkspaceDetail {
            row,
            issues: Arc::clone(&self.load.issues),
        })
    }
}

impl FileLocksState {
    /// 按稳定 PID + 路径键从当前快照取得详情。
    pub fn selected_detail(&self) -> Option<SelectedWorkspaceDetail> {
        let key = self.selection.selected()?;
        let row = self
            .load
            .rows
            .iter()
            .find(|row| row.pid == key.pid && row.path == key.path)
            .cloned()
            .map_or(SelectedWorkspaceRow::Stale, SelectedWorkspaceRow::File);
        Some(SelectedWorkspaceDetail {
            row,
            issues: Arc::clone(&self.load.issues),
        })
    }
}
