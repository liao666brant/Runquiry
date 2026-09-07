//! File Locks 工作区的两种模式、锁优先与稳定选择。

use std::{cmp::Ordering, collections::HashMap, path::PathBuf, sync::Arc};

use runquiry_core::{
    CapabilityStatus, FileInventoryEntry, Generation, Inspection, LockMetadata, LockMode, LockType,
    Pid,
};

use super::{LoadPresentation, StableSelection};

/// 文件清单稳定键；同 PID/路径只对应一个选择对象。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FileKey {
    /// 持有者 PID。
    pub pid: Pid,
    /// FD 目标或锁路径。
    pub path: PathBuf,
}

/// 文件清单模式。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FileLockMode {
    /// 仅真实锁。
    #[default]
    Locked,
    /// 全部打开文件；同 PID/路径有锁时仅保留锁行。
    AllOpen,
}

/// 文件清单排序列。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FileLockSort {
    /// 路径、PID 稳定默认顺序。
    #[default]
    Path,
    /// PID。
    Pid,
    /// 进程名。
    Process,
    /// FD。
    Fd,
    /// 锁类型。
    Type,
    /// 锁模式。
    Mode,
}

/// File Locks 页面纯状态。
#[derive(Clone, Debug, Default)]
pub struct FileLocksState {
    /// 加载状态与不可变快照。
    pub load: LoadPresentation<FileInventoryEntry>,
    /// 当前模式。
    pub mode: FileLockMode,
    /// 稳定选择。
    pub selection: StableSelection<FileKey>,
    filter: String,
    sort: FileLockSort,
    ascending: bool,
    visible_indices: Arc<[usize]>,
}

impl FileLocksState {
    /// 应用当前代际快照；旧结果被拒绝。
    pub fn apply(
        &mut self,
        generation: Generation,
        capability: &CapabilityStatus,
        inspection: Inspection<Arc<[FileInventoryEntry]>>,
    ) -> bool {
        if !self.load.apply(generation, capability, inspection) {
            return false;
        }
        self.rebuild();
        true
    }

    /// 设置仅锁/全部打开文件模式。
    pub fn set_mode(&mut self, mode: FileLockMode) -> Generation {
        self.mode = mode;
        let generation = self.load.advance();
        self.rebuild();
        generation
    }

    /// 设置不区分大小写筛选。
    pub fn set_filter(&mut self, filter: impl Into<String>) -> Generation {
        self.filter = filter.into();
        let generation = self.load.advance();
        self.rebuild();
        generation
    }

    /// 设置排序。
    pub fn set_sort(&mut self, sort: FileLockSort, ascending: bool) -> Generation {
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
    pub fn row(&self, visible_index: usize) -> Option<&FileInventoryEntry> {
        self.visible_indices
            .get(visible_index)
            .and_then(|index| self.load.rows.get(*index))
    }

    /// `DataTable` 选择事件对应的稳定领域键。
    pub fn key_at(&self, visible_index: usize) -> Option<FileKey> {
        self.row(visible_index).map(key)
    }

    /// 当前筛选文本。
    pub fn filter(&self) -> &str {
        &self.filter
    }

    fn rebuild(&mut self) {
        let needle = self.filter.to_lowercase();
        let preferred = preferred_rows(&self.load.rows);
        let mut indices: Vec<_> = preferred
            .into_values()
            .filter(|index| {
                let row = &self.load.rows[*index];
                (self.mode == FileLockMode::AllOpen || row.lock.is_some())
                    && matches_filter(row, &needle)
            })
            .collect();
        indices.sort_by(|a, b| self.compare(&self.load.rows[*a], &self.load.rows[*b]));
        self.visible_indices = indices.into();
        let present = self.selection.selected().is_none_or(|selected| {
            self.load
                .rows
                .iter()
                .any(|row| row.pid == selected.pid && row.path == selected.path)
        });
        self.selection.reconcile_presence(present);
    }

    fn compare(&self, a: &FileInventoryEntry, b: &FileInventoryEntry) -> Ordering {
        let ordering = match self.sort {
            FileLockSort::Path => a.path.cmp(&b.path).then(a.pid.cmp(&b.pid)),
            FileLockSort::Pid => a.pid.cmp(&b.pid).then(a.path.cmp(&b.path)),
            FileLockSort::Process => a.process.cmp(&b.process),
            FileLockSort::Fd => a.fd.cmp(&b.fd),
            FileLockSort::Type => lock_type(a.lock).cmp(lock_type(b.lock)),
            FileLockSort::Mode => lock_mode(a.lock).cmp(lock_mode(b.lock)),
        };
        if self.ascending {
            ordering
        } else {
            ordering.reverse()
        }
    }
}

fn preferred_rows(rows: &[FileInventoryEntry]) -> HashMap<FileKey, usize> {
    let mut preferred: HashMap<FileKey, usize> = HashMap::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        let row_key = key(row);
        preferred
            .entry(row_key)
            .and_modify(|current| {
                let current_row = &rows[*current];
                let replace = match (current_row.lock, row.lock) {
                    (None, Some(_)) => true,
                    (None, None) => row.fd < current_row.fd,
                    (Some(_), None | Some(_)) => false,
                };
                if replace {
                    *current = index;
                }
            })
            .or_insert(index);
    }
    preferred
}

fn key(row: &FileInventoryEntry) -> FileKey {
    FileKey {
        pid: row.pid,
        path: row.path.clone(),
    }
}

fn matches_filter(row: &FileInventoryEntry, needle: &str) -> bool {
    needle.is_empty()
        || row.path.to_string_lossy().to_lowercase().contains(needle)
        || row.process.to_lowercase().contains(needle)
        || row.pid.to_string().contains(needle)
        || row.fd.is_some_and(|fd| fd.to_string().contains(needle))
        || lock_type(row.lock).to_lowercase().contains(needle)
        || lock_mode(row.lock).to_lowercase().contains(needle)
}

pub(super) fn lock_type(lock: Option<LockMetadata>) -> &'static str {
    match lock.map(|metadata| metadata.lock_type) {
        None => "打开",
        Some(LockType::Posix) => "POSIX",
        Some(LockType::Flock) => "FLOCK",
        Some(LockType::Ofdlck) => "OFDLCK",
        Some(LockType::Other) => "其他锁",
    }
}

pub(super) fn lock_mode(lock: Option<LockMetadata>) -> &'static str {
    match lock.map(|metadata| metadata.mode) {
        None => "—",
        Some(LockMode::Read) => "读",
        Some(LockMode::Write) => "写",
        Some(LockMode::ReadWrite) => "读写",
    }
}
