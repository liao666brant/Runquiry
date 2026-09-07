//! Processes 工作区的纯状态与集成 seam。

use std::ops::Range;
use std::sync::Arc;

use runquiry_core::{Generation, ProcessIdentity, ProcessSummary};

use super::DetailPrivacySession;

/// 详情请求标识；结果必须同时匹配身份与 generation。
#[derive(Clone, Debug)]
pub struct DetailRequest {
    identity: ProcessIdentity,
    generation: Generation,
}

impl DetailRequest {
    /// 请求的进程身份。
    pub const fn identity(&self) -> &ProcessIdentity {
        &self.identity
    }

    /// 请求代际。
    pub const fn generation(&self) -> Generation {
        self.generation
    }
}

/// 列表选择变化的可观察结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionChange {
    /// 选择成功。
    Selected,
    /// 同一完整身份在新快照中仍存在。
    Preserved,
    /// PID 相同但启动时间变化。
    PidReused,
    /// 原目标已经消失。
    Disappeared,
    /// 选择已清除或索引无效。
    Cleared,
}

/// 大列表共享存储；可见窗口只借用切片，不复制全量数据。
#[derive(Clone, Debug)]
pub struct ProcessRows {
    rows: Arc<[ProcessSummary]>,
}

impl ProcessRows {
    /// 接管共享快照。
    pub const fn new(rows: Arc<[ProcessSummary]>) -> Self {
        Self { rows }
    }

    /// 共享底层快照，供 `DataTable` delegate 按索引读取。
    pub const fn rows(&self) -> &Arc<[ProcessSummary]> {
        &self.rows
    }

    /// 总行数。
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// 借用被虚拟列表请求的范围。
    pub fn visible(&self, range: Range<usize>) -> &[ProcessSummary] {
        let start = range.start.min(self.rows.len());
        let end = range.end.min(self.rows.len()).max(start);
        &self.rows[start..end]
    }
}

/// Processes 页面可被 shell 组合的状态。
#[derive(Clone, Debug)]
pub struct ProcessesState {
    rows: ProcessRows,
    selected: Option<ProcessIdentity>,
    detail_generation: Generation,
    privacy: DetailPrivacySession,
}

impl ProcessesState {
    /// 以一次列表快照建立页面状态。
    pub const fn new(rows: Arc<[ProcessSummary]>) -> Self {
        Self {
            rows: ProcessRows::new(rows),
            selected: None,
            detail_generation: Generation::first(),
            privacy: DetailPrivacySession::new(),
        }
    }

    /// 当前共享行模型。
    pub const fn rows(&self) -> &ProcessRows {
        &self.rows
    }

    /// 当前稳定选择。
    pub const fn selected(&self) -> Option<&ProcessIdentity> {
        self.selected.as_ref()
    }

    /// 当前详情隐私会话。
    pub const fn privacy(&self) -> &DetailPrivacySession {
        &self.privacy
    }

    /// 可变详情隐私会话。
    pub const fn privacy_mut(&mut self) -> &mut DetailPrivacySession {
        &mut self.privacy
    }

    /// 按当前行位置选择，但保存完整领域身份而非 index。
    pub fn select_row(&mut self, row_ix: usize) -> SelectionChange {
        let Some(row) = self.rows.rows().get(row_ix) else {
            return self.clear_selection();
        };
        self.selected = Some(row.identity.clone());
        let _ = self.detail_generation.next();
        self.privacy.reset();
        SelectionChange::Selected
    }

    /// 用新列表快照替换数据，并按完整身份恢复选择。
    pub fn replace_rows(&mut self, rows: Arc<[ProcessSummary]>) -> SelectionChange {
        self.rows = ProcessRows::new(rows);
        let _ = self.detail_generation.next();
        self.privacy.reset();
        let Some(selected) = self.selected.as_ref() else {
            return SelectionChange::Cleared;
        };
        let matching_pid = self
            .rows
            .rows()
            .iter()
            .find(|row| row.identity.pid() == selected.pid());
        match matching_pid {
            Some(row) if selected.same_process(&row.identity) => {
                self.selected = Some(row.identity.clone());
                SelectionChange::Preserved
            }
            Some(_) => {
                self.selected = None;
                SelectionChange::PidReused
            }
            None => {
                self.selected = None;
                SelectionChange::Disappeared
            }
        }
    }

    /// 清除选择与当前详情会话。
    pub fn clear_selection(&mut self) -> SelectionChange {
        self.selected = None;
        let _ = self.detail_generation.next();
        self.privacy.reset();
        SelectionChange::Cleared
    }

    /// 为当前选择发起新详情请求。
    pub fn begin_detail(&mut self) -> Option<DetailRequest> {
        let identity = self.selected.clone()?;
        Some(DetailRequest {
            identity,
            generation: self.detail_generation.next(),
        })
    }

    /// 判断异步结果能否应用到当前详情面板。
    pub fn accepts(&self, request: &DetailRequest) -> bool {
        !request.generation.is_stale(self.detail_generation)
            && self
                .selected
                .as_ref()
                .is_some_and(|selected| selected.same_process(&request.identity))
    }
}
