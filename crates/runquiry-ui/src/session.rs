//! 每工作区会话状态：LoadState、generation、选择、排序与筛选。
//!
//! 四个工作区各自独立维护一整套会话状态（[`WorkspaceSession`]），由
//! [`AppSession`] 聚合；主题与语言不在此结构内，切换它们不可能影响会话。
//! 本模块是纯逻辑（无 GPUI 类型），刷新门控与代际语义来自
//! [`runquiry_core::refresh`]：手工刷新与自动刷新都走
//! [`WorkspaceSession::try_refresh`] 这一条通道，in-flight 期间被拒绝；
//! 携带旧代际的列表/详情结果必须由调用方经 [`WorkspaceSession::is_current`]
//! 判定后丢弃。

use std::time::Duration;

use runquiry_core::{Generation, RefreshGate};

use crate::state::DataState;

/// 产品工作区（与 witr TUI 的四个标签页一一对应）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WorkspaceId {
    /// Processes 工作区。
    #[default]
    Processes,
    /// Ports 工作区。
    Ports,
    /// Containers 工作区。
    Containers,
    /// File Locks 工作区。
    FileLocks,
}

impl WorkspaceId {
    /// 全部工作区，顺序即侧栏与快捷键（Ctrl/Cmd+1..4）顺序。
    pub const ALL: [Self; 4] = [
        Self::Processes,
        Self::Ports,
        Self::Containers,
        Self::FileLocks,
    ];

    /// 稳定键名（设置文件 `last_workspace` 与侧栏元素 ID 使用）。
    pub const fn key(self) -> &'static str {
        match self {
            Self::Processes => "processes",
            Self::Ports => "ports",
            Self::Containers => "containers",
            Self::FileLocks => "file-locks",
        }
    }

    /// 从稳定键名解析工作区。
    pub fn parse(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|id| id.key() == key)
    }

    /// 在 [`WorkspaceId::ALL`] 中的位置；越界回退到第一项。
    pub fn at(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or(Self::Processes)
    }
}

/// 一次排序的方向与列。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortOrder {
    /// 排序列（列的稳定 ID）。
    pub column: String,
    /// true 为升序。
    pub ascending: bool,
}

/// 一个工作区的会话状态。
#[derive(Debug)]
pub struct WorkspaceSession {
    /// 数据区域当前状态；会话默认 Loading，产品壳层在采集器不可用时映射为
    /// [`DataState::Unsupported`]。
    pub data_state: DataState,
    /// 当前有效代际：只有携带该代际的列表/详情结果可以应用。
    pub generation: Generation,
    /// 稳定选择（领域 ID；`None` 表示未选择）。
    pub selection: Option<String>,
    /// 当前排序；`None` 表示未排序（默认顺序）。
    pub sort: Option<SortOrder>,
    /// 当前筛选内容；`None` 表示无筛选。筛选内容是调查输入，不进设置文件。
    pub filter: Option<String>,
    gate: RefreshGate,
}

impl Default for WorkspaceSession {
    fn default() -> Self {
        Self {
            data_state: DataState::Loading,
            generation: Generation::first(),
            selection: None,
            sort: None,
            filter: None,
            gate: RefreshGate::new(),
        }
    }
}

impl WorkspaceSession {
    /// 尝试发起一次刷新（手工与自动共用通道）。
    ///
    /// in-flight 期间返回 `None`（禁止重入）；成功时递增代际并返回新代际，
    /// 结果必须携带该代际回传。
    pub const fn try_refresh(&mut self) -> Option<Generation> {
        let mut candidate = self.generation;
        let candidate = candidate.next();
        if !self.gate.try_begin(candidate) {
            return None;
        }
        self.generation = candidate;
        Some(candidate)
    }

    /// 结束指定代际的刷新并计入耗时样本（驱动 3–30 秒自适应间隔）。
    ///
    /// 过期完成信号返回 `false`，不得释放随后开始的新刷新。
    pub fn finish_refresh(&mut self, generation: Generation, took: Duration) -> bool {
        self.gate.finish(generation, took)
    }

    /// 丢弃指定代际的刷新（如工作区已切换），不产生耗时样本。
    ///
    /// 过期中止信号返回 `false`，不得释放当前刷新。
    pub fn abort_refresh(&mut self, generation: Generation) -> bool {
        self.gate.abort(generation)
    }

    /// 刷新是否在进行中。
    pub const fn is_refreshing(&self) -> bool {
        self.gate.is_busy()
    }

    /// 当前自适应刷新间隔。
    pub const fn refresh_interval(&self) -> Duration {
        self.gate.interval()
    }

    /// 结果是否仍然有效（代际未变）；过期结果必须丢弃。
    pub const fn is_current(&self, generation: Generation) -> bool {
        !generation.is_stale(self.generation)
    }

    /// 更新选择；选择变化会递增代际（旧列表与旧详情一并失效）。
    ///
    /// 相同选择不递增；返回变化后的新代际。
    pub fn select(&mut self, selection: Option<String>) -> Option<Generation> {
        if self.selection == selection {
            return None;
        }
        self.selection = selection;
        Some(self.generation.next())
    }

    /// 更新筛选；筛选变化递增代际并返回新代际。
    pub fn set_filter(&mut self, filter: Option<String>) -> Generation {
        self.filter = filter;
        self.generation.next()
    }

    /// 更新排序；排序变化递增代际并返回新代际。
    pub fn set_sort(&mut self, sort: Option<SortOrder>) -> Generation {
        self.sort = sort;
        self.generation.next()
    }

    /// 更新数据状态（由真实采集结果或能力结论驱动）。
    pub const fn set_data_state(&mut self, data_state: DataState) {
        self.data_state = data_state;
    }
}

/// 全应用会话状态：四个工作区 + 当前工作区。
#[derive(Debug)]
pub struct AppSession {
    sessions: [WorkspaceSession; WorkspaceId::ALL.len()],
    active: WorkspaceId,
}

impl Default for AppSession {
    fn default() -> Self {
        Self {
            sessions: std::array::from_fn(|_| WorkspaceSession::default()),
            active: WorkspaceId::default(),
        }
    }
}

impl AppSession {
    /// 创建会话：起步在 Processes 工作区，各工作区均为初始等待态。
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前工作区。
    pub const fn active(&self) -> WorkspaceId {
        self.active
    }

    /// 读取指定工作区的会话。
    pub const fn session(&self, workspace: WorkspaceId) -> &WorkspaceSession {
        &self.sessions[workspace as usize]
    }

    /// 修改指定工作区的会话（工作区之间完全隔离）。
    pub const fn session_mut(&mut self, workspace: WorkspaceId) -> &mut WorkspaceSession {
        &mut self.sessions[workspace as usize]
    }

    /// 当前工作区的会话。
    pub const fn active_session(&self) -> &WorkspaceSession {
        self.session(self.active)
    }

    /// 修改当前工作区的会话。
    pub const fn active_session_mut(&mut self) -> &mut WorkspaceSession {
        let active = self.active;
        self.session_mut(active)
    }

    /// 切换工作区并递增目标工作区的代际：切换瞬间的在途结果一律视为过期。
    pub const fn switch_workspace(&mut self, workspace: WorkspaceId) -> Generation {
        self.active = workspace;
        self.sessions[workspace as usize].generation.next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 断言一次刷新成功并返回代际（`expect`/`unwrap` 在本仓库为 deny，
    /// 测试用显式断言展开）。
    fn must_refresh(session: &mut WorkspaceSession, what: &str) -> Generation {
        let generation = session.try_refresh();
        assert!(generation.is_some(), "{what}");
        generation.unwrap_or_default()
    }

    /// 工作区键名稳定且可往返（设置文件 `last_workspace` 依赖）。
    #[test]
    fn workspace_keys_round_trip() {
        for id in WorkspaceId::ALL {
            assert_eq!(WorkspaceId::parse(id.key()), Some(id));
        }
        assert_eq!(
            WorkspaceId::parse("file-locks"),
            Some(WorkspaceId::FileLocks)
        );
        assert_eq!(WorkspaceId::parse("lock"), None);
        assert_eq!(WorkspaceId::at(3), WorkspaceId::FileLocks);
        assert_eq!(WorkspaceId::at(9), WorkspaceId::Processes);
    }

    /// 工作区之间状态隔离：改一个的 filter/selection 不影响其他。
    #[test]
    fn workspaces_are_isolated() {
        let mut app = AppSession::new();
        let processes = app.active();
        let others: Vec<_> = WorkspaceId::ALL
            .into_iter()
            .filter(|id| *id != processes)
            .collect();
        let other_generations: Vec<_> = others
            .iter()
            .map(|id| app.session(*id).generation)
            .collect();

        app.session_mut(processes).set_filter(Some("cargo".into()));
        app.session_mut(processes).select(Some("pid-42".into()));

        for (other, generation) in others.iter().zip(other_generations) {
            let session = app.session(*other);
            assert_eq!(
                session.filter, None,
                "{other:?} 不应被 {processes:?} 的筛选影响"
            );
            assert_eq!(
                session.selection, None,
                "{other:?} 不应被 {processes:?} 的选择影响"
            );
            assert_eq!(
                session.sort, None,
                "{other:?} 不应被 {processes:?} 的排序影响"
            );
            assert_eq!(
                session.generation, generation,
                "{other:?} 的代际不应被 {processes:?} 影响"
            );
        }
    }

    /// 切换工作区只递增目标工作区的代际，且不清空任何工作区的会话。
    #[test]
    fn switching_workspace_bumps_only_target() {
        let mut app = AppSession::new();
        app.session_mut(WorkspaceId::Ports)
            .set_filter(Some("443".into()));
        let ports_gen = app.session(WorkspaceId::Ports).generation;

        app.switch_workspace(WorkspaceId::Ports);
        assert!(app.session(WorkspaceId::Ports).generation > ports_gen);
        assert_eq!(app.session(WorkspaceId::Ports).filter, Some("443".into()));

        let processes_gen = app.session(WorkspaceId::Processes).generation;
        app.switch_workspace(WorkspaceId::Containers);
        assert_eq!(
            app.session(WorkspaceId::Processes).generation,
            processes_gen
        );
        assert_eq!(app.active(), WorkspaceId::Containers);
    }

    /// 刷新重入被拒绝；手工与自动共用一条通道。
    #[test]
    fn refresh_is_not_reentrant() {
        let mut session = WorkspaceSession::default();
        let generation = must_refresh(&mut session, "首次刷新应成功");
        assert!(session.is_refreshing());
        // 自动路径与手工路径都是同一个 try_refresh：重入一律 None。
        assert_eq!(session.try_refresh(), None);
        assert_eq!(session.try_refresh(), None);

        assert!(session.finish_refresh(generation, Duration::from_millis(50)));
        assert!(!session.is_refreshing());
        assert!(session.try_refresh().is_some());
        // 结果只对发起时的代际有效。
        assert!(!session.is_current(generation));
    }

    /// 选择/筛选/排序变化使在途刷新结果过期（旧 generation 丢弃）。
    #[test]
    fn selection_change_discards_inflight_results() {
        let mut session = WorkspaceSession::default();
        let refresh_generation = must_refresh(&mut session, "刷新应成功");

        let selected = session.select(Some("pid-7".into()));
        assert!(selected.is_some(), "选择变化应递增代际");
        let new_generation = selected.unwrap_or_default();
        assert!(session.is_current(new_generation));
        assert!(
            !session.is_current(refresh_generation),
            "旧代际的列表必须被丢弃"
        );

        let generation = new_generation;
        let filtered = session.set_filter(Some("ssh".into()));
        assert!(session.is_current(filtered), "新筛选后的代际有效");
        assert!(
            !session.is_current(generation),
            "筛选前的在途结果必须被丢弃"
        );
        let sorted = session.set_sort(Some(SortOrder {
            column: "pid".into(),
            ascending: true,
        }));
        assert!(session.is_current(sorted), "新排序后的代际有效");
        assert!(!session.is_current(filtered), "排序前筛选后的代际已过期");

        // 刷新结束后自适应间隔已生效；abort 后立即可再次发起。
        assert!(session.finish_refresh(refresh_generation, Duration::from_millis(10)));
        let next = must_refresh(&mut session, "上次刷新完成后应允许再次刷新");
        assert!(session.abort_refresh(next));
        assert!(!session.is_refreshing());
    }

    /// 中止 g1 后开始 g2，晚到的 g1 完成信号不得释放 g2。
    #[test]
    fn stale_refresh_completion_does_not_release_current_generation() {
        let mut session = WorkspaceSession::default();
        let g1 = must_refresh(&mut session, "g1 应开始");
        assert!(session.abort_refresh(g1));
        let g2 = must_refresh(&mut session, "g2 应在 g1 中止后开始");

        assert!(!session.abort_refresh(g1));
        assert!(session.is_refreshing(), "g1 中止信号不得释放 g2");
        assert!(!session.finish_refresh(g1, Duration::from_millis(50)));
        assert!(session.is_refreshing(), "g1 完成信号不得释放 g2");
        assert_eq!(session.try_refresh(), None, "g2 仍在途时必须拒绝重入");

        assert!(session.finish_refresh(g2, Duration::from_millis(50)));
        assert!(!session.is_refreshing());
    }

    /// 主题/语言不进会话：切换工作区不触碰会话内容（generation 递增是设计语义）。
    #[test]
    fn session_has_no_theme_or_language_state() {
        let mut app = AppSession::new();
        let generation = app.session(WorkspaceId::Processes).generation;
        let before = format!("{:?}", app.session(WorkspaceId::Processes).data_state);
        app.switch_workspace(WorkspaceId::FileLocks);
        app.switch_workspace(WorkspaceId::Processes);

        let session = app.session(WorkspaceId::Processes);
        assert_eq!(session.selection, None);
        assert_eq!(session.sort, None);
        assert_eq!(session.filter, None);
        assert_eq!(format!("{:?}", session.data_state), before);
        assert!(
            session.generation > generation,
            "切回本工作区会递增代际（在途结果过期），但不得触碰会话内容"
        );
    }
}
