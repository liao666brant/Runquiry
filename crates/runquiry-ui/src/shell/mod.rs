//! 产品应用壳层：固定侧栏（四个工作区）、工具栏、主数据区、详情区与状态栏。
//!
//! 状态归属：每工作区会话（LoadState/generation/选择/排序/筛选）在
//! [`AppSession`]；主题与语言只是壳层字段，切换不会触碰会话。壳层对平台
//! 的唯一假设是「采集器不可用」：主数据区以 Unsupported 状态呈现（见
//! locales 的 `main.collector_unavailable`），自动刷新循环与手工刷新共用
//! [`WorkspaceSession::try_refresh`] 一条通道，在途期间拒绝重入。

mod render;

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use gpui::{Context, EventEmitter, FocusHandle, Focusable, Task};
use gpui_component::ThemeMode;
use runquiry_core::Generation;

use crate::debounce::{DETAIL_DEBOUNCE_MS, DetailDebounce};
use crate::locale::{Lang, set_language};
use crate::session::{AppSession, WorkspaceId};
use crate::theme;

/// 壳层的 key context（快捷键绑定的作用域）。
pub const KEY_CONTEXT: &str = "RunquiryShell";

/// 主数据区与详情区的宽度配比（DESIGN.md §7：1100px 及以上为 65/35）。
pub(crate) const MAIN_RATIO: f32 = 65.;
pub(crate) const DETAIL_RATIO: f32 = 35.;

/// 手写 `Debug`：`Task`/`FocusHandle` 不参与诊断语义，只输出状态字段。
#[allow(clippy::missing_fields_in_debug)]
impl std::fmt::Debug for AppShell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppShell")
            .field("active", &self.session.active())
            .field("theme", &self.theme)
            .field("lang", &self.lang)
            .field("refreshing", &self.session.active_session().is_refreshing())
            .field("debounce_pending", &self.debounce.is_pending())
            .finish()
    }
}

/// 壳层对外的设置变化事件；持久化由装配层（runquiry-app）监听并落盘。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellEvent {
    /// 主题已切换。
    ThemeChanged(ThemeMode),
    /// 语言已切换。
    LanguageChanged(Lang),
    /// 当前工作区已切换。
    WorkspaceChanged(WorkspaceId),
}

/// 壳层快捷键动作（`actions!` 生成的类型无法逐个补文档，统一豁免）。
///
/// `pub` 供装配层（runquiry-app）做键位绑定；动作的处理逻辑在壳层
/// `render` 的 `on_action` 里。
#[allow(missing_docs, clippy::derive_partial_eq_without_eq)]
pub mod actions {
    gpui::actions!(runquiry_ui, [RefreshWorkspace]);
}

/// 壳层的启动设置：由装配层从设置文件加载后传入，字段一次初始化到位
/// （不触发 [`ShellEvent`]，启动路径不回写设置文件）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShellStartup {
    /// 初始主题（设置文件未提供时由调用方给默认值）。
    pub theme: ThemeMode,
    /// 初始语言。
    pub language: Lang,
    /// 启动后停留的工作区。
    pub workspace: WorkspaceId,
}

/// 产品应用壳层视图。
pub struct AppShell {
    pub(crate) session: AppSession,
    pub(crate) theme: ThemeMode,
    pub(crate) lang: Lang,
    pub(crate) sidebar_focus: FocusHandle,
    pub(crate) toolbar_focus: FocusHandle,
    pub(crate) main_focus: FocusHandle,
    pub(crate) detail_focus: FocusHandle,
    /// 详情加载 500ms 防抖（见 [`DetailDebounce`]）。
    debounce: DetailDebounce,
    /// 当前刷新的代际与开始时刻，用于拒绝过期完成信号并提供真实耗时样本。
    refresh_started: Option<(Generation, Instant)>,
    /// 自动刷新循环：必须保存在字段里维持生命周期（丢弃即取消）。
    _auto_refresh_task: Task<()>,
    /// 详情防抖到期任务：新选择会取消旧任务（替换字段）。
    /// 下划线命名表达「仅维持生命周期、从不读值」；赋值即使用，故豁免该 lint。
    #[allow(clippy::used_underscore_binding)]
    _detail_task: Option<Task<()>>,
}

impl EventEmitter<ShellEvent> for AppShell {}

impl Focusable for AppShell {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.toolbar_focus.clone()
    }
}

impl AppShell {
    /// 创建壳层并启动自动刷新循环。
    pub fn new(startup: ShellStartup, cx: &mut Context<'_, Self>) -> Self {
        let auto_refresh_task = spawn_auto_refresh(cx);
        let mut session = AppSession::new();
        // 启动工作区：非默认工作区时经 switch_workspace 落位（递增一次代际，
        // 启动瞬间没有在途结果，无副作用）。
        session.switch_workspace(startup.workspace);
        Self {
            session,
            theme: startup.theme,
            lang: startup.language,
            sidebar_focus: cx.focus_handle().tab_index(15),
            toolbar_focus: cx.focus_handle(),
            main_focus: cx.focus_handle().tab_index(16),
            detail_focus: cx.focus_handle().tab_index(17),
            debounce: DetailDebounce::new(),
            refresh_started: None,
            _auto_refresh_task: auto_refresh_task,
            _detail_task: None,
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

    /// 切换主题：只影响壳层主题字段，不会触碰任何工作区会话。
    pub fn set_theme(
        &mut self,
        mode: ThemeMode,
        window: &mut gpui::Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.theme == mode {
            return;
        }
        theme::apply(mode, Some(window), cx);
        self.theme = mode;
        cx.emit(ShellEvent::ThemeChanged(mode));
        cx.notify();
    }

    /// 切换语言：切换全局 locale 后必须显式 `cx.notify()`（locale 不是
    /// GPUI 追踪的状态）。
    pub fn set_language(&mut self, lang: Lang, cx: &mut Context<'_, Self>) {
        if self.lang == lang {
            return;
        }
        set_language(lang);
        self.lang = lang;
        cx.emit(ShellEvent::LanguageChanged(lang));
        cx.notify();
    }

    /// 切换工作区：中止离开工作区的在途刷新与详情防抖，并递增目标工作区的
    /// 代际（切换瞬间的在途结果一律过期）。
    pub fn switch_workspace(&mut self, workspace: WorkspaceId, cx: &mut Context<'_, Self>) {
        if self.session.active() == workspace {
            return;
        }
        if let Some((generation, _)) = self.refresh_started.take() {
            self.session.active_session_mut().abort_refresh(generation);
        }
        self.debounce.cancel();
        self.session.switch_workspace(workspace);
        cx.emit(ShellEvent::WorkspaceChanged(workspace));
        cx.notify();
    }

    /// 更新工作区的选择：递增代际并请求详情防抖（500ms 后加载详情）。
    ///
    /// 平台端口尚未接入，真实表格要等 B5/B6；但取消/失效语义在这里完整
    /// 落地：新的选择会取消旧的防抖任务。
    pub fn select(
        &mut self,
        workspace: WorkspaceId,
        selection: Option<String>,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(generation) = self.session.session_mut(workspace).select(selection) else {
            return;
        };
        if workspace == self.session.active() {
            self.debounce.request(generation, now_ms());
            self._detail_task = Some(spawn_detail_timer(cx));
        }
    }

    /// 发起一次当前工作区的刷新：自动与手工共用这条通道，在途时静默拒绝。
    pub fn refresh_active(&mut self, cx: &mut Context<'_, Self>) {
        let session = self.session.active_session_mut();
        let Some(generation) = session.try_refresh() else {
            return; // 重入被拒绝：不重复发起
        };
        self.refresh_started = Some((generation, Instant::now()));
        // 平台采集端口尚未接入（B2/B3 交付后由装配层注入）：本轮没有可采集
        // 的数据源，以真实耗时收尾，让自适应间隔状态机保持运行，而不是伪造
        // 一份调查结果。主数据区的诚实等待态见 render。
        let Some((generation, started)) = self.refresh_started.take() else {
            return;
        };
        let took = started.elapsed();
        if self
            .session
            .active_session_mut()
            .finish_refresh(generation, took)
        {
            cx.notify();
        }
    }

    /// 防抖到期后加载详情：代际已过期的请求一律丢弃，旧详情不得覆盖新选择。
    fn poll_detail(&mut self, workspace: WorkspaceId, cx: &mut Context<'_, Self>) {
        let Some(generation) = self.debounce.poll(now_ms()) else {
            return;
        };
        if !self.session.session(workspace).is_current(generation) {
            return; // stale：期间选择/筛选/工作区已变化
        }
        // 平台端口尚未接入：详情区维持占位态（见 render），不伪造数据。
        cx.notify();
    }

    /// 当前工作区的自适应刷新间隔。
    pub(crate) const fn active_interval(&self) -> Duration {
        self.session.active_session().refresh_interval()
    }
}

/// 单调时钟读数（毫秒）。防抖只关心相对时序，锚点取进程内首次调用时刻。
fn now_ms() -> u64 {
    static ANCHOR: OnceLock<Instant> = OnceLock::new();
    u64::try_from(ANCHOR.get_or_init(Instant::now).elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// 启动自动刷新循环：按当前自适应间隔休眠，到期后回前台刷新当前工作区。
fn spawn_auto_refresh(cx: &Context<'_, AppShell>) -> Task<()> {
    cx.spawn(async move |shell, cx| {
        loop {
            let interval = shell
                .update(cx, |shell, _cx| shell.active_interval())
                .unwrap_or(Duration::from_secs(3));
            cx.background_executor().timer(interval).await;
            // Entity 已释放（窗口关闭）则退出循环。
            if shell.update(cx, AppShell::refresh_active).is_err() {
                break;
            }
        }
    })
}

/// 详情防抖的到期定时任务：到期后由前台轮询防抖器并检查代际。
fn spawn_detail_timer(cx: &Context<'_, AppShell>) -> Task<()> {
    cx.spawn(async move |shell, cx| {
        // 略超窗口，保证到期时 poll 一定能取到。
        cx.background_executor()
            .timer(Duration::from_millis(DETAIL_DEBOUNCE_MS + 5))
            .await;
        shell
            .update(cx, |shell, cx| {
                let workspace = shell.active_workspace();
                shell.poll_detail(workspace, cx);
            })
            .ok();
    })
}
