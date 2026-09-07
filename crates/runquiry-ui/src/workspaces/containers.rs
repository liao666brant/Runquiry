//! Containers 工作区的领域行、筛选、排序与 fallback 语义。

use std::{cmp::Ordering, sync::Arc, time::SystemTime};

use runquiry_core::{
    CapabilityStatus, ContainerKey, ContainerSummary, Generation, Inspection, Pid,
};

use super::{LoadPresentation, StableSelection};

/// 仅携带已验证宿主 PID 的容器行。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContainerRow {
    /// 运行时清单信息。
    pub summary: ContainerSummary,
    /// 经 `ContainerProcessVerifier` 验证的宿主 PID。
    pub verified_host_pid: Option<Pid>,
}

impl ContainerRow {
    /// 由 app 装配层在完成归属验证后构造。
    pub const fn new(summary: ContainerSummary, verified_host_pid: Option<Pid>) -> Self {
        Self {
            summary,
            verified_host_pid,
        }
    }

    /// 稳定选择键。
    pub const fn key(&self) -> &ContainerKey {
        &self.summary.key
    }

    /// 未验证或验证失败时必须继续展示容器 fallback。
    pub const fn uses_fallback(&self) -> bool {
        self.verified_host_pid.is_none()
    }
}

/// 容器排序列。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ContainerSort {
    /// runtime + id 稳定默认顺序。
    #[default]
    Key,
    /// 名称。
    Name,
    /// 状态。
    Status,
    /// 健康状态。
    Health,
    /// 镜像。
    Image,
    /// 已验证宿主 PID。
    HostPid,
    /// 启动时间。
    StartedAt,
}

/// Containers 页面纯状态。
#[derive(Clone, Debug, Default)]
pub struct ContainersState {
    /// 加载状态与不可变快照。
    pub load: LoadPresentation<ContainerRow>,
    /// 稳定选择。
    pub selection: StableSelection<ContainerKey>,
    filter: String,
    sort: ContainerSort,
    ascending: bool,
    visible_indices: Arc<[usize]>,
}

impl ContainersState {
    /// 应用当前代际快照；旧结果被拒绝。
    pub fn apply(
        &mut self,
        generation: Generation,
        capability: &CapabilityStatus,
        inspection: Inspection<Arc<[ContainerRow]>>,
    ) -> bool {
        if !self.load.apply(generation, capability, inspection) {
            return false;
        }
        self.rebuild();
        true
    }

    /// 设置不区分大小写筛选。
    pub fn set_filter(&mut self, filter: impl Into<String>) -> Generation {
        self.filter = filter.into();
        let generation = self.load.advance();
        self.rebuild();
        generation
    }

    /// 设置排序。
    pub fn set_sort(&mut self, sort: ContainerSort, ascending: bool) -> Generation {
        self.sort = sort;
        self.ascending = ascending;
        let generation = self.load.advance();
        self.rebuild();
        generation
    }

    /// 当前可见行索引。
    pub fn visible_indices(&self) -> &[usize] {
        &self.visible_indices
    }

    /// 通过可见行索引访问领域行。
    pub fn row(&self, visible_index: usize) -> Option<&ContainerRow> {
        self.visible_indices
            .get(visible_index)
            .and_then(|index| self.load.rows.get(*index))
    }

    /// `DataTable` 选择事件对应的稳定领域键。
    pub fn key_at(&self, visible_index: usize) -> Option<ContainerKey> {
        self.row(visible_index).map(|row| row.key().clone())
    }

    /// 当前筛选文本。
    pub fn filter(&self) -> &str {
        &self.filter
    }

    fn rebuild(&mut self) {
        let needle = self.filter.to_lowercase();
        let mut indices: Vec<_> = self
            .load
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| matches_filter(row, &needle))
            .map(|(index, _)| index)
            .collect();
        indices.sort_by(|a, b| self.compare(&self.load.rows[*a], &self.load.rows[*b]));
        self.visible_indices = indices.into();
        self.selection
            .reconcile(self.load.rows.iter().map(ContainerRow::key));
    }

    fn compare(&self, a: &ContainerRow, b: &ContainerRow) -> Ordering {
        let ordering = match self.sort {
            ContainerSort::Key => a.summary.key.cmp(&b.summary.key),
            ContainerSort::Name => a.summary.name.cmp(&b.summary.name),
            ContainerSort::Status => a.summary.status.cmp(&b.summary.status),
            ContainerSort::Health => a.summary.health.cmp(&b.summary.health),
            ContainerSort::Image => a.summary.image.cmp(&b.summary.image),
            ContainerSort::HostPid => a.verified_host_pid.cmp(&b.verified_host_pid),
            ContainerSort::StartedAt => compare_time(a.summary.started_at, b.summary.started_at),
        };
        if self.ascending {
            ordering
        } else {
            ordering.reverse()
        }
    }
}

fn matches_filter(row: &ContainerRow, needle: &str) -> bool {
    needle.is_empty()
        || row.summary.key.runtime.to_lowercase().contains(needle)
        || row.summary.key.id.to_lowercase().contains(needle)
        || optional_contains(row.summary.name.as_deref(), needle)
        || optional_contains(row.summary.status.as_deref(), needle)
        || optional_contains(row.summary.health.as_deref(), needle)
        || optional_contains(row.summary.image.as_deref(), needle)
        || row
            .verified_host_pid
            .is_some_and(|pid| pid.to_string().contains(needle))
}

fn optional_contains(value: Option<&str>, needle: &str) -> bool {
    value.is_some_and(|value| value.to_lowercase().contains(needle))
}

fn compare_time(a: Option<SystemTime>, b: Option<SystemTime>) -> Ordering {
    a.cmp(&b)
}
