//! Ports 工作区的领域行、筛选、排序与稳定选择。

use std::{cmp::Ordering, sync::Arc};

use runquiry_core::{CapabilityStatus, Generation, Inspection, OpenPortEntry, Pid, Port, Protocol};

use super::{LoadPresentation, StableSelection};

/// 端口表稳定复合键；不依赖行索引。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PortKey {
    /// 协议。
    pub protocol: Protocol,
    /// 本地地址。
    pub address: String,
    /// 本地端口。
    pub port: Port,
    /// 状态。
    pub state: String,
    /// 可选属主。
    pub pid: Option<Pid>,
}

/// app 装配层补入进程名后的端口行。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortRow {
    /// 平台端口条目。
    pub entry: OpenPortEntry,
    /// 属主进程名；无属主或详情不可得时为空。
    pub process: Option<String>,
    /// 是否监听所有接口。
    pub public_bind: bool,
}

impl PortRow {
    /// 从领域条目构造；公开绑定由地址语义确定。
    pub fn new(entry: OpenPortEntry, process: Option<String>) -> Self {
        let public_bind = matches!(entry.address.as_str(), "0.0.0.0" | "::");
        Self {
            entry,
            process,
            public_bind,
        }
    }

    /// 稳定领域键。
    pub fn key(&self) -> PortKey {
        PortKey {
            protocol: self.entry.protocol,
            address: self.entry.address.clone(),
            port: self.entry.port,
            state: self.entry.state.clone(),
            pid: self.entry.pid,
        }
    }
}

/// 端口范围模式。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PortMode {
    /// 仅监听 socket。
    #[default]
    Listening,
    /// 全部 socket。
    All,
}

/// 端口排序列。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PortSort {
    /// 协议、端口、地址稳定默认顺序。
    #[default]
    Protocol,
    /// 地址。
    Address,
    /// 端口。
    Port,
    /// 状态。
    State,
    /// PID，无属主排在末尾。
    Pid,
    /// 进程名。
    Process,
    /// 公开绑定优先。
    PublicBind,
}

/// Ports 页面纯状态；DataTable 只消费 `visible_indices` 的可见切片。
#[derive(Clone, Debug, Default)]
pub struct PortsState {
    /// 加载状态与不可变快照。
    pub load: LoadPresentation<PortRow>,
    /// 模式。
    pub mode: PortMode,
    /// 稳定选择。
    pub selection: StableSelection<PortKey>,
    filter: String,
    sort: PortSort,
    ascending: bool,
    visible_indices: Arc<[usize]>,
}

impl PortsState {
    /// 应用当前代际快照；旧结果被拒绝。
    pub fn apply(
        &mut self,
        generation: Generation,
        capability: &CapabilityStatus,
        inspection: Inspection<Arc<[PortRow]>>,
    ) -> bool {
        if !self.load.apply(generation, capability, inspection) {
            return false;
        }
        self.rebuild();
        true
    }

    /// 设置仅监听/全部模式。
    pub fn set_mode(&mut self, mode: PortMode) -> Generation {
        self.mode = mode;
        let generation = self.load.advance();
        self.rebuild();
        generation
    }

    /// 设置不区分大小写的文本筛选。
    pub fn set_filter(&mut self, filter: impl Into<String>) -> Generation {
        self.filter = filter.into();
        let generation = self.load.advance();
        self.rebuild();
        generation
    }

    /// 设置排序。
    pub fn set_sort(&mut self, sort: PortSort, ascending: bool) -> Generation {
        self.sort = sort;
        self.ascending = ascending;
        let generation = self.load.advance();
        self.rebuild();
        generation
    }

    /// 当前可见行索引；DataTable 按需访问，不复制领域行。
    pub fn visible_indices(&self) -> &[usize] {
        &self.visible_indices
    }

    /// 通过可见行索引访问领域行。
    pub fn row(&self, visible_index: usize) -> Option<&PortRow> {
        self.visible_indices
            .get(visible_index)
            .and_then(|index| self.load.rows.get(*index))
    }

    /// `DataTable` 选择事件对应的稳定领域键。
    pub fn key_at(&self, visible_index: usize) -> Option<PortKey> {
        self.row(visible_index).map(PortRow::key)
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
            .filter(|(_, row)| self.mode == PortMode::All || is_listening(&row.entry.state))
            .filter(|(_, row)| matches_filter(row, &needle))
            .map(|(index, _)| index)
            .collect();
        indices.sort_by(|a, b| self.compare(&self.load.rows[*a], &self.load.rows[*b]));
        self.visible_indices = indices.into();
        let present = self
            .selection
            .selected()
            .is_none_or(|selected| self.load.rows.iter().any(|row| row.matches_key(selected)));
        self.selection.reconcile_presence(present);
    }

    fn compare(&self, a: &PortRow, b: &PortRow) -> Ordering {
        let ordering = match self.sort {
            PortSort::Protocol => {
                protocol_name(a.entry.protocol).cmp(protocol_name(b.entry.protocol))
            }
            PortSort::Address => a.entry.address.cmp(&b.entry.address),
            PortSort::Port => a.entry.port.cmp(&b.entry.port),
            PortSort::State => a.entry.state.cmp(&b.entry.state),
            PortSort::Pid => a.entry.pid.cmp(&b.entry.pid),
            PortSort::Process => a.process.cmp(&b.process),
            PortSort::PublicBind => a.public_bind.cmp(&b.public_bind).reverse(),
        };
        if self.ascending {
            ordering
        } else {
            ordering.reverse()
        }
    }
}

impl PortRow {
    fn matches_key(&self, key: &PortKey) -> bool {
        self.entry.protocol == key.protocol
            && self.entry.address == key.address
            && self.entry.port == key.port
            && self.entry.state == key.state
            && self.entry.pid == key.pid
    }
}

const fn is_listening(state: &str) -> bool {
    state.eq_ignore_ascii_case("listen") || state.eq_ignore_ascii_case("listening")
}

fn matches_filter(row: &PortRow, needle: &str) -> bool {
    needle.is_empty()
        || row.entry.address.to_lowercase().contains(needle)
        || row.entry.state.to_lowercase().contains(needle)
        || row.entry.port.to_string().contains(needle)
        || row
            .entry
            .pid
            .is_some_and(|pid| pid.to_string().contains(needle))
        || row
            .process
            .as_deref()
            .is_some_and(|process| process.to_lowercase().contains(needle))
}

pub(super) const fn protocol_name(protocol: Protocol) -> &'static str {
    match protocol {
        Protocol::Tcp => "TCP",
        Protocol::Tcp6 => "TCP6",
        Protocol::Udp => "UDP",
        Protocol::Udp6 => "UDP6",
        Protocol::Unix => "UNIX",
    }
}
