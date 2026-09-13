//! Processes 的 gpui-component 虚拟化表格适配器。

use std::collections::BTreeSet;
use std::ops::Range;
use std::sync::Arc;

use gpui_kit::component::{
    ActiveTheme as _, Sizable as _, Size,
    menu::{PopupMenu, PopupMenuItem},
    table::{Column, ColumnSort, DataTable, TableDelegate, TableEvent, TableState},
    tooltip::Tooltip,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use runquiry_core::{ProcessIdentity, ProcessSummary};
use rust_i18n::t;

use crate::format::{format_bytes, format_timestamp};
use crate::workspaces::ColumnVisibility;
use crate::{DataState, StateView, workspace_state_copy};

use super::{ProcessRows, SurfaceState};

/// 进程排序键（表头点击选择；`cpu`/`memory` 对应资源列）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProcessSortKey {
    /// 按 PID 排序。
    Pid,
    /// 按 CPU% 排序（默认，降序）。
    #[default]
    Cpu,
    /// 按内存排序。
    Memory,
}

impl ProcessSortKey {
    /// 排序键对应的表格列 key。
    pub const fn column_key(self) -> &'static str {
        match self {
            Self::Pid => "pid",
            Self::Cpu => "cpu",
            Self::Memory => "memory",
        }
    }
}

/// 进程排序状态。默认 CPU% 降序（witr 行为）；值缺失（首样本）的行无论
/// 方向恒排尾部并按 PID 升序。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessSort {
    /// 排序键。
    pub key: ProcessSortKey,
    /// 是否降序。
    pub descending: bool,
}

impl Default for ProcessSort {
    fn default() -> Self {
        Self {
            key: ProcessSortKey::Cpu,
            descending: true,
        }
    }
}

/// `DataTable` 的领域数据适配器。
#[derive(Debug)]
pub struct ProcessTableDelegate {
    rows: ProcessRows,
    columns: Vec<Column>,
    visibility: ColumnVisibility,
    sort: ProcessSort,
    /// 壳层句柄：右键菜单项经其回发动作请求（走既有确认流）。
    shell: Option<gpui_kit::WeakEntity<crate::shell::AppShell>>,
    /// 右键菜单可见的关闭类动作快照（随 Processes 刷新的逐动作能力）。
    kill_available: bool,
    kill_tree_available: bool,
    visible: Range<usize>,
    surface: SurfaceState,
}

impl ProcessTableDelegate {
    /// 全量列定义（含被隐藏的列）；列设置弹层按此渲染选项。
    pub fn column_defs() -> Vec<Column> {
        vec![
            Column::new("name", t!("column.name").to_string())
                .width(px(180.))
                .min_width(px(120.)),
            Column::new("pid", t!("column.pid").to_string())
                .width(px(88.))
                .sortable()
                .text_right(),
            Column::new("cpu", t!("column.cpu").to_string())
                .width(px(88.))
                .sortable()
                .text_right(),
            Column::new("memory", t!("column.memory").to_string())
                .width(px(160.))
                .sortable()
                .text_right(),
            Column::new("started", t!("column.started").to_string()).width(px(180.)),
            Column::new("user", t!("column.user").to_string()).width(px(140.)),
            Column::new("health", t!("column.health").to_string()).width(px(110.)),
            Column::new("command", t!("column.command").to_string())
                .width(px(280.))
                .min_width(px(160.)),
        ]
    }

    /// 以共享快照与初始排序建立表格；DataTable 自身负责虚拟化与键盘导航。
    pub fn new(rows: Arc<[ProcessSummary]>, sort: ProcessSort) -> Self {
        let surface = if rows.is_empty() {
            SurfaceState::Empty
        } else {
            SurfaceState::Ready
        };
        Self {
            rows: ProcessRows::new(rows),
            columns: Self::column_defs(),
            visibility: ColumnVisibility::new(),
            sort,
            shell: None,
            kill_available: false,
            kill_tree_available: false,
            visible: 0..0,
            surface,
        }
    }

    /// 注入壳层句柄（右键菜单回发动作请求用）。
    pub fn set_shell(&mut self, shell: gpui_kit::WeakEntity<crate::shell::AppShell>) {
        self.shell = Some(shell);
    }

    /// 同步右键菜单可见的关闭类动作快照。
    pub fn set_kill_actions(&mut self, kill: bool, kill_tree: bool) {
        self.kill_available = kill;
        self.kill_tree_available = kill_tree;
    }

    /// 当前排序状态。
    pub const fn sort(&self) -> ProcessSort {
        self.sort
    }

    /// 表头点击后的新排序（DataTable 的 Default 视为回到默认 CPU% 降序）。
    fn sort_from_column_sort(key: ProcessSortKey, sort: ColumnSort) -> ProcessSort {
        match sort {
            ColumnSort::Descending => ProcessSort {
                key,
                descending: true,
            },
            ColumnSort::Ascending => ProcessSort {
                key,
                descending: false,
            },
            ColumnSort::Default => ProcessSort::default(),
        }
    }

    /// 整体替换隐藏列集合（来自壳层的持久化状态）。
    pub fn set_hidden(&mut self, hidden: BTreeSet<String>) {
        self.visibility.set_hidden(hidden);
    }

    /// 替换共享快照，不复制每一行。
    pub fn replace_rows(&mut self, rows: Arc<[ProcessSummary]>) {
        self.rows = ProcessRows::new(rows);
        self.visible = 0..0;
    }

    /// 同步壳层的排序状态（表头箭头与排序语义的单一事实源在壳层）。
    pub fn set_sort(&mut self, sort: ProcessSort) {
        self.sort = sort;
    }

    /// 更新列表表面的加载/能力/错误状态。
    pub fn set_surface(&mut self, surface: SurfaceState) {
        self.surface = surface;
    }

    /// 按当前 locale 重建列标题，不触碰行快照、列显隐、排序与表面状态。
    pub fn relocalize(&mut self) {
        let rows = Arc::clone(self.rows.rows());
        let surface = self.surface.clone();
        let hidden = self.visibility.hidden().clone();
        let sort = self.sort;
        let shell = self.shell.clone();
        let (kill_available, kill_tree_available) = (self.kill_available, self.kill_tree_available);
        *self = Self::new(rows, sort);
        self.surface = surface;
        self.visibility.set_hidden(hidden);
        self.shell = shell;
        self.kill_available = kill_available;
        self.kill_tree_available = kill_tree_available;
    }

    /// 当前可见列（渲染与列设置弹层共用）。
    fn visible_columns(&self) -> Vec<Column> {
        self.visibility.visible(&self.columns).cloned().collect()
    }

    /// 按当前表格行位置取得完整领域身份。
    pub fn identity_at(&self, row_ix: usize) -> Option<&ProcessIdentity> {
        self.rows.rows().get(row_ix).map(|row| &row.identity)
    }

    /// 按领域身份生成与当前排序位置无关的 GPUI 行 ID。
    pub fn stable_row_id_at(&self, row_ix: usize) -> Option<SharedString> {
        self.rows.rows().get(row_ix).map(Self::row_id)
    }

    /// `DataTable` 最近报告的可见范围。
    pub const fn visible_range(&self) -> &Range<usize> {
        &self.visible
    }

    fn row_id(row: &ProcessSummary) -> SharedString {
        let started = row
            .identity
            .start_time()
            .and_then(|time| time.duration_since(std::time::SystemTime::UNIX_EPOCH).ok())
            .map_or(0, |duration| duration.as_nanos());
        SharedString::from(format!("process-{}-{started}", row.identity.pid()))
    }
}

impl TableDelegate for ProcessTableDelegate {
    fn columns_count(&self, _: &App) -> usize {
        self.visibility.visible_count(&self.columns)
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        let mut column = self
            .visible_columns()
            .into_iter()
            .nth(col_ix)
            .unwrap_or_else(|| Column::new("unknown", "—"));
        // 当前排序列的表头箭头（DataTable 依据该字段渲染并驱动点击循环）。
        if column.key == self.sort.key.column_key() {
            column.sort = Some(if self.sort.descending {
                ColumnSort::Descending
            } else {
                ColumnSort::Ascending
            });
        }
        column
    }

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: ColumnSort,
        _: &mut Window,
        cx: &mut Context<'_, TableState<Self>>,
    ) {
        let Some(key) = self
            .visible_columns()
            .get(col_ix)
            .map(|column| column.key.to_string())
        else {
            return;
        };
        let key = match key.as_str() {
            "pid" => ProcessSortKey::Pid,
            "cpu" => ProcessSortKey::Cpu,
            "memory" => ProcessSortKey::Memory,
            _ => return,
        };
        self.sort = Self::sort_from_column_sort(key, sort);
        // 通知壳层按新排序重排行数据（过滤与排序在壳层完成）。
        cx.emit(TableEvent::SelectColumn(col_ix));
        cx.notify();
    }

    /// 行右键菜单：关闭类动作入口（逐动作能力快照门禁；菜单项携带右键所在
    /// 行的身份，经壳层句柄回发 `request_process_action_for`，进入既有两步
    /// 确认流，不依赖选中态的事件次序）。
    fn context_menu(
        &mut self,
        row_ix: usize,
        menu: PopupMenu,
        _: &mut Window,
        _: &mut Context<'_, TableState<Self>>,
    ) -> PopupMenu {
        let Some(identity) = self.identity_at(row_ix).cloned() else {
            return menu;
        };
        let Some(shell) = self.shell.clone() else {
            return menu;
        };
        let menu = if self.kill_available {
            menu.item(
                PopupMenuItem::new(t!("actions.kill").to_string()).on_click({
                    let shell = shell.clone();
                    let identity = identity.clone();
                    move |_, _window, app| {
                        let _ = shell.update_in(app, |shell, window, cx| {
                            shell.request_process_action_for(
                                &identity,
                                runquiry_core::ProcessAction::Kill,
                                window,
                                cx,
                            );
                        });
                    }
                }),
            )
        } else {
            menu
        };
        if self.kill_tree_available {
            menu.item(
                PopupMenuItem::new(t!("actions.kill_tree").to_string()).on_click({
                    move |_, _window, app| {
                        let _ = shell.update_in(app, |shell, window, cx| {
                            shell.request_process_action_for(
                                &identity,
                                runquiry_core::ProcessAction::KillTree,
                                window,
                                cx,
                            );
                        });
                    }
                }),
            )
        } else {
            menu
        }
    }

    fn loading(&self, _: &App) -> bool {
        self.surface == SurfaceState::Loading
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        _: &mut Context<'_, TableState<Self>>,
    ) -> impl IntoElement {
        let (state, boundary_reason) = match &self.surface {
            SurfaceState::Loading | SurfaceState::Sampling => (DataState::Loading, None),
            SurfaceState::Empty | SurfaceState::Ready | SurfaceState::Partial { .. } => {
                (DataState::Empty, None)
            }
            SurfaceState::Error { .. } => (DataState::Error, None),
            SurfaceState::PermissionDenied => (DataState::PermissionDenied, None),
            SurfaceState::Unsupported { reason } => (DataState::Unsupported, Some(reason.as_str())),
            SurfaceState::Unavailable { reason } => (DataState::Unavailable, Some(reason.as_str())),
        };
        let (title, description) = workspace_state_copy(state);
        StateView::new(state, title)
            .description(description)
            .when_some(boundary_reason, |view, reason| {
                // 能力/环境边界必须解释原因：UI 不改写平台结论。
                view.note(reason)
            })
            .into_any_element()
    }

    fn render_tr(
        &mut self,
        row_ix: usize,
        _: &mut Window,
        _: &mut Context<'_, TableState<Self>>,
    ) -> gpui_kit::Stateful<gpui_kit::Div> {
        let id = self
            .stable_row_id_at(row_ix)
            .unwrap_or_else(|| SharedString::from("process-missing"));
        div().id(id)
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<'_, TableState<Self>>,
    ) -> impl IntoElement {
        let Some(row) = self.rows.rows().get(row_ix) else {
            return div().into_any_element();
        };
        // 可见索引 → 列 key：隐藏列不影响后续列的语义分派。
        let Some(key) = self
            .visible_columns()
            .get(col_ix)
            .map(|column| column.key.clone())
        else {
            return div().into_any_element();
        };
        match key.as_ref() {
            // 名称：进程短名（comm/可执行名），与 witr Name 列同源。
            "name" => div()
                .overflow_hidden()
                .text_ellipsis()
                .text_color(cx.theme().foreground)
                .child(row.command.clone())
                .into_any_element(),
            "pid" => div()
                .font_family(cx.theme().mono_font_family.clone())
                .child(row.identity.pid().to_string())
                .into_any_element(),
            "cpu" => div()
                .child(render_cpu_percent(row.cpu_percent))
                .into_any_element(),
            "memory" => div()
                .child(render_memory(row.memory_rss_bytes, row.memory_percent))
                .into_any_element(),
            "started" => div()
                .child(format_timestamp(row.identity.start_time()))
                .into_any_element(),
            "user" => div()
                .child(row.user.clone().unwrap_or_else(|| String::from("—")))
                .into_any_element(),
            "health" => div().child(row.health.as_str()).into_any_element(),
            // 命令：完整命令行（与短名不同源）；不可得时回退短名，不再杜撰后缀。
            "command" => {
                let command_line = row
                    .command_line
                    .clone()
                    .unwrap_or_else(|| row.command.clone());
                div()
                    .id(("command", u64::from(row.identity.pid().get())))
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_color(cx.theme().foreground)
                    .tooltip({
                        let command_line = command_line.clone();
                        move |window, cx| Tooltip::new(command_line.clone()).build(window, cx)
                    })
                    .child(command_line)
                    .into_any_element()
            }
            _ => div().into_any_element(),
        }
    }

    fn visible_rows_changed(
        &mut self,
        visible_range: Range<usize>,
        _: &mut Window,
        _: &mut Context<'_, TableState<Self>>,
    ) {
        self.visible = visible_range;
    }
}

/// CPU% 单元格：一位小数（多核可超 100，witr 同语义），首样本前为占位符。
fn render_cpu_percent(cpu_percent: Option<f64>) -> String {
    cpu_percent.map_or_else(
        || String::from(crate::format::UNAVAILABLE),
        |percent| format!("{percent:.1}%"),
    )
}

/// 内存单元格：witr 风格 `452.5 MB (1.4%)`；总量不可得时只显示字节。
fn render_memory(rss_bytes: Option<u64>, memory_percent: Option<f64>) -> String {
    let Some(rss) = rss_bytes else {
        return String::from(crate::format::UNAVAILABLE);
    };
    match memory_percent {
        Some(percent) => format!("{} ({percent:.1}%)", format_bytes(rss)),
        None => format_bytes(rss),
    }
}

/// shell 可直接嵌入的 Processes 数据表元素。
#[derive(IntoElement)]
pub struct ProcessTable {
    state: Entity<TableState<ProcessTableDelegate>>,
}

impl std::fmt::Debug for ProcessTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessTable").finish_non_exhaustive()
    }
}

impl ProcessTable {
    /// 组合已有 TableState；订阅 `TableEvent` 的生命周期由 shell 拥有。
    pub fn new(state: &Entity<TableState<ProcessTableDelegate>>) -> Self {
        Self {
            state: state.clone(),
        }
    }
}

impl RenderOnce for ProcessTable {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        DataTable::new(&self.state)
            .with_size(Size::Small)
            .stripe(true)
            .bordered(true)
            .scrollbar_visible(true, true)
    }
}
