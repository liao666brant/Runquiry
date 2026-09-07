//! 三个清单工作区共享的加载状态与稳定选择语义。

use std::sync::Arc;

use runquiry_core::{CapabilityStatus, DiagnosticCode, DiagnosticIssue, Generation, Inspection};

use crate::DataState;

/// 数据区呈现状态；部分成功由 `issues` 与 `capability_note` 横幅表达。
#[derive(Clone, Debug)]
pub struct LoadPresentation<T> {
    /// 当前代际。
    pub generation: Generation,
    /// 六态中的基础状态。
    pub state: DataState,
    /// 不复制领域行的不可变快照。
    pub rows: Arc<[T]>,
    /// 需要在表格上方展示的诊断。
    pub issues: Arc<[DiagnosticIssue]>,
    /// `Partial` 能力原因。
    pub capability_note: Option<Arc<str>>,
}

impl<T> Default for LoadPresentation<T> {
    fn default() -> Self {
        Self {
            generation: Generation::first(),
            state: DataState::Loading,
            rows: Arc::default(),
            issues: Arc::default(),
            capability_note: None,
        }
    }
}

impl<T> LoadPresentation<T> {
    /// 只接受当前代际结果，并将能力与 Inspection 映射为 UI 状态。
    pub fn apply(
        &mut self,
        generation: Generation,
        capability: &CapabilityStatus,
        inspection: Inspection<Arc<[T]>>,
    ) -> bool {
        if generation.is_stale(self.generation) {
            return false;
        }
        let capability_note = match capability {
            CapabilityStatus::Partial(reason) => Some(Arc::from(reason.as_str())),
            CapabilityStatus::Supported
            | CapabilityStatus::Unsupported(_)
            | CapabilityStatus::Unavailable(_) => None,
        };
        let issues: Arc<[DiagnosticIssue]> = inspection.issues.into();
        let rows = inspection.data.unwrap_or_default();
        let state = map_state(capability, &rows, &issues);
        self.rows = rows;
        self.issues = issues;
        self.capability_note = capability_note;
        self.state = state;
        true
    }

    /// 推进代际，供筛选、排序、模式与刷新变更使用。
    pub const fn advance(&mut self) -> Generation {
        self.generation.next()
    }

    /// 是否应显示部分成功横幅。
    pub fn is_partial(&self) -> bool {
        self.state == DataState::Ready
            && (!self.issues.is_empty() || self.capability_note.is_some())
    }
}

fn map_state<T>(
    capability: &CapabilityStatus,
    rows: &[T],
    issues: &[DiagnosticIssue],
) -> DataState {
    if matches!(capability, CapabilityStatus::Unsupported(_))
        || issues
            .iter()
            .any(|issue| issue.code() == DiagnosticCode::Unsupported)
    {
        return DataState::Unsupported;
    }
    let denied = issues
        .iter()
        .any(|issue| issue.code() == DiagnosticCode::PermissionDenied);
    let has_data = !rows.is_empty();
    if has_data {
        return DataState::Ready;
    }
    if denied {
        return DataState::PermissionDenied;
    }
    if matches!(capability, CapabilityStatus::Unavailable(_)) || !issues.is_empty() {
        return DataState::Error;
    }
    DataState::Empty
}

/// 用领域 ID 保存选择；行消失时保留选择并标记 stale，不跳到其他行。
#[derive(Clone, Debug)]
pub struct StableSelection<K> {
    selected: Option<K>,
    stale: bool,
}

impl<K> Default for StableSelection<K> {
    fn default() -> Self {
        Self {
            selected: None,
            stale: false,
        }
    }
}

impl<K: Eq + Clone> StableSelection<K> {
    /// 选择领域对象。
    pub fn select(&mut self, key: Option<K>) {
        self.selected = key;
        self.stale = false;
    }

    /// 按新快照核对选择是否仍存在。
    pub fn reconcile<'a>(&mut self, keys: impl IntoIterator<Item = &'a K>)
    where
        K: 'a,
    {
        self.stale = self
            .selected
            .as_ref()
            .is_some_and(|selected| !keys.into_iter().any(|key| key == selected));
    }

    /// 按调用方已完成的领域查找结果更新 stale 状态。
    pub const fn reconcile_presence(&mut self, present: bool) {
        self.stale = self.selected.is_some() && !present;
    }

    /// 当前稳定领域 ID。
    pub const fn selected(&self) -> Option<&K> {
        self.selected.as_ref()
    }

    /// 目标是否已从最新快照消失。
    pub const fn is_stale(&self) -> bool {
        self.stale
    }
}
