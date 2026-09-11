//! 产品应用壳层：平台数据、调查入口与四个工作区的生命周期所有者。

mod bindings;
mod compact_detail;
mod data;
mod interactions;
mod process_actions;
mod refresh;
mod render;
mod render_detail;
mod render_evidence;
mod render_issues;
mod render_process_actions;
mod render_workspace;
mod render_workspace_detail;
mod workspace_interactions;

use std::sync::Arc;
use std::time::Instant;

use gpui_kit::component::{ThemeMode, input::InputState};
use gpui_kit::{
    AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, Subscription, Task,
    Window,
};
use runquiry_core::{Analysis, CapabilityStatus, Generation, InspectError, Inspection};
use rust_i18n::t;

use crate::backend::{InvestigationTarget, WorkspaceBackend, WorkspaceResultGate};
use crate::locale::{Lang, set_language};
use crate::processes::{DetailRequest, ProcessActionFlow, QueryOutcome, TargetKind};
use crate::session::{AppSession, WorkspaceId};
use crate::theme;
use bindings::table_subscriptions;
use data::ShellData;
use refresh::spawn_auto_refresh;

/// 壳层快捷键作用域。
pub const KEY_CONTEXT: &str = "RunquiryShell";
pub(crate) const MAIN_RATIO: f32 = 65.;
pub(crate) const DETAIL_RATIO: f32 = 35.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// 需要由应用装配层持久化的设置变化。
pub enum ShellEvent {
    /// 主题变化。
    ThemeChanged(ThemeMode),
    /// 语言变化。
    LanguageChanged(Lang),
    /// 当前工作区变化。
    WorkspaceChanged(WorkspaceId),
}

#[allow(missing_docs, clippy::derive_partial_eq_without_eq)]
pub mod actions {
    gpui_kit::actions!(
        runquiry_ui,
        [
            RefreshWorkspace,
            FocusQuery,
            Workspace1,
            Workspace2,
            Workspace3,
            Workspace4,
            OpenProcessActions,
            KillProcess,
            TerminateProcess,
            PauseProcess,
            ResumeProcess,
            ReniceProcess,
            CloseProcessActions
        ]
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// 首帧应用的持久化启动设置。
pub struct ShellStartup {
    /// 初始主题。
    pub theme: ThemeMode,
    /// 初始语言。
    pub language: Lang,
    /// 初始工作区。
    pub workspace: WorkspaceId,
}

/// Runquiry 产品窗口的根内容视图。
pub struct AppShell {
    pub(crate) session: AppSession,
    pub(crate) theme: ThemeMode,
    pub(crate) lang: Lang,
    pub(crate) sidebar_focus: FocusHandle,
    pub(crate) toolbar_focus: FocusHandle,
    pub(crate) main_focus: FocusHandle,
    pub(crate) detail_focus: FocusHandle,
    pub(crate) query_input: Entity<InputState>,
    pub(crate) filter_input: Entity<InputState>,
    pub(crate) renice_input: Entity<InputState>,
    pub(crate) target_kind: TargetKind,
    pub(crate) query_outcome: Option<QueryOutcome<InvestigationTarget>>,
    pub(crate) query_error: Option<InspectError>,
    pub(crate) analysis: Option<Inspection<Analysis>>,
    data: ShellData,
    backend: Arc<dyn WorkspaceBackend>,
    process_action_capability: CapabilityStatus,
    process_action_flow: ProcessActionFlow,
    process_action_menu_open: bool,
    query_generation: Generation,
    detail_request: Option<DetailRequest>,
    refresh_started: Option<(WorkspaceResultGate, Instant)>,
    _subscriptions: Vec<Subscription>,
    _auto_refresh_task: Task<()>,
    refresh_task: Option<Task<()>>,
    detail_task: Option<Task<()>>,
    process_action_task: Option<Task<()>>,
    detail_loading: bool,
}

impl EventEmitter<ShellEvent> for AppShell {}

impl std::fmt::Debug for AppShell {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AppShell")
            .field("workspace", &self.active_workspace())
            .field("theme", &self.theme)
            .field("language", &self.lang)
            .finish_non_exhaustive()
    }
}

impl Focusable for AppShell {
    fn focus_handle(&self, _: &gpui_kit::App) -> FocusHandle {
        self.toolbar_focus.clone()
    }
}

impl AppShell {
    /// 创建持有真实后端与四个虚拟表的产品壳层。
    pub fn new(
        startup: ShellStartup,
        backend: Arc<dyn WorkspaceBackend>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let mut session = AppSession::new();
        session.switch_workspace(startup.workspace);
        let data = ShellData::new(window, cx);
        let query_input = cx
            .new(|cx| InputState::new(window, cx).placeholder(t!("query.placeholder").to_string()));
        let filter_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t!("filter.placeholder").to_string())
        });
        let renice_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value("0")
                .placeholder(t!("actions.renice.placeholder").to_string())
        });
        let subscriptions = table_subscriptions(&data, &query_input, &filter_input, window, cx);
        let process_action_capability = backend.process_control_capability();
        Self {
            session,
            theme: startup.theme,
            lang: startup.language,
            sidebar_focus: cx.focus_handle().tab_index(15),
            toolbar_focus: cx.focus_handle(),
            main_focus: cx.focus_handle().tab_index(16),
            detail_focus: cx.focus_handle().tab_index(17),
            query_input,
            filter_input,
            renice_input,
            target_kind: TargetKind::Name,
            query_outcome: None,
            query_error: None,
            analysis: None,
            data,
            backend,
            process_action_capability,
            process_action_flow: ProcessActionFlow::new(),
            process_action_menu_open: false,
            query_generation: Generation::first(),
            detail_request: None,
            refresh_started: None,
            _subscriptions: subscriptions,
            _auto_refresh_task: spawn_auto_refresh(cx),
            refresh_task: None,
            detail_task: None,
            process_action_task: None,
            detail_loading: false,
        }
    }

    /// 当前工作区。
    pub const fn active_workspace(&self) -> WorkspaceId {
        self.session.active()
    }
    /// 当前主题。
    pub const fn theme(&self) -> ThemeMode {
        self.theme
    }
    /// 当前语言。
    pub const fn language(&self) -> Lang {
        self.lang
    }

    /// 切换主题且保留所有工作区状态。
    pub fn set_theme(&mut self, mode: ThemeMode, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.theme == mode {
            return;
        }
        theme::apply(mode, Some(window), cx);
        self.theme = mode;
        cx.emit(ShellEvent::ThemeChanged(mode));
        cx.notify();
    }

    /// 切换语言且保留所有工作区状态。
    pub fn set_language(&mut self, lang: Lang, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.lang == lang {
            return;
        }
        set_language(lang);
        self.lang = lang;
        self.query_input.update(cx, |input, cx| {
            input.set_placeholder(t!("query.placeholder").to_string(), window, cx);
        });
        self.filter_input.update(cx, |input, cx| {
            input.set_placeholder(t!("filter.placeholder").to_string(), window, cx);
        });
        self.renice_input.update(cx, |input, cx| {
            input.set_placeholder(t!("actions.renice.placeholder").to_string(), window, cx);
        });
        self.data.relocalize(cx);
        cx.emit(ShellEvent::LanguageChanged(lang));
        cx.notify();
    }

    /// 切换工作区并立即加载其真实快照。
    pub fn switch_workspace(&mut self, workspace: WorkspaceId, cx: &mut Context<'_, Self>) {
        if self.session.active() == workspace {
            return;
        }
        if let Some((gate, _)) = self.refresh_started.take() {
            self.session
                .session_mut(gate.workspace())
                .abort_refresh(gate.generation());
        }
        self.session.switch_workspace(workspace);
        self.analysis = None;
        self.detail_loading = false;
        self.process_action_flow.invalidate_context();
        self.process_action_menu_open = false;
        cx.emit(ShellEvent::WorkspaceChanged(workspace));
        cx.notify();
        self.refresh_active(cx);
    }
}
