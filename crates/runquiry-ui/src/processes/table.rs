//! Processes 的 gpui-component 虚拟化表格适配器。

use std::ops::Range;
use std::sync::Arc;

use gpui_kit::component::{
    ActiveTheme as _, Sizable as _, Size,
    table::{Column, DataTable, TableDelegate, TableState},
    tooltip::Tooltip,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use runquiry_core::{ProcessIdentity, ProcessSummary};
use rust_i18n::t;

use crate::{DataState, StateView, workspace_state_copy};

use super::{ProcessRows, SurfaceState};

/// `DataTable` 的领域数据适配器。
#[derive(Debug)]
pub struct ProcessTableDelegate {
    rows: ProcessRows,
    columns: Vec<Column>,
    visible: Range<usize>,
    surface: SurfaceState,
}

impl ProcessTableDelegate {
    /// 以共享快照建立表格；DataTable 自身负责虚拟化与键盘导航。
    pub fn new(rows: Arc<[ProcessSummary]>) -> Self {
        let surface = if rows.is_empty() {
            SurfaceState::Empty
        } else {
            SurfaceState::Ready
        };
        Self {
            rows: ProcessRows::new(rows),
            columns: vec![
                Column::new("command", t!("column.command").to_string())
                    .width(px(280.))
                    .min_width(px(160.)),
                Column::new("pid", t!("column.pid").to_string())
                    .width(px(88.))
                    .text_right(),
                Column::new("user", t!("column.user").to_string()).width(px(160.)),
                Column::new("health", t!("column.health").to_string()).width(px(120.)),
            ],
            visible: 0..0,
            surface,
        }
    }

    /// 替换共享快照，不复制每一行。
    pub fn replace_rows(&mut self, rows: Arc<[ProcessSummary]>) {
        self.rows = ProcessRows::new(rows);
        self.visible = 0..0;
    }

    /// 更新列表表面的加载/能力/错误状态。
    pub fn set_surface(&mut self, surface: SurfaceState) {
        self.surface = surface;
    }

    /// 按当前 locale 重建列标题，不触碰行快照与表面状态。
    pub fn relocalize(&mut self) {
        let rows = Arc::clone(self.rows.rows());
        let surface = self.surface.clone();
        *self = Self::new(rows);
        self.surface = surface;
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
        self.columns.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        self.columns
            .get(col_ix)
            .cloned()
            .unwrap_or_else(|| Column::new("unknown", "—"))
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
        let command = row.command.clone();
        match col_ix {
            0 => div()
                .id(("command", u64::from(row.identity.pid().get())))
                .overflow_hidden()
                .text_ellipsis()
                .text_color(cx.theme().foreground)
                .tooltip({
                    let command = command.clone();
                    move |window, cx| Tooltip::new(command.clone()).build(window, cx)
                })
                .child(command)
                .into_any_element(),
            1 => div()
                .font_family(cx.theme().mono_font_family.clone())
                .child(row.identity.pid().to_string())
                .into_any_element(),
            2 => div()
                .child(row.user.clone().unwrap_or_else(|| String::from("—")))
                .into_any_element(),
            3 => div().child(row.health.as_str()).into_any_element(),
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
