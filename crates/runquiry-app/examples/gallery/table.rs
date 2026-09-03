//! gallery 的数据表：列定义、合成数据源与行渲染。
//!
//! 从 `main.rs` 原样搬入（A4 拆分），行为不变。
//!
//! 本模块是 example 内部的私有模块：条目用 `pub(crate)` 暴露给 `main.rs`，
//! 这里显式豁免 `redundant_pub_crate`（否则与 `unreachable_pub` 互相冲突）。

#![allow(clippy::redundant_pub_crate)]

use gpui::{
    App, Context, InteractiveElement as _, IntoElement, ParentElement as _, Styled as _, Window,
    div, px,
};
use gpui_component::{
    ActiveTheme as _,
    table::{Column, TableDelegate, TableState},
};

use crate::data;
use runquiry_ui::{DataState, Dict, Lang, StateView};

/// 数据表的合成数据源。
pub(crate) struct GalleryTable {
    pub(crate) lang: Lang,
    pub(crate) state: DataState,
    pub(crate) columns: Vec<Column>,
}

impl GalleryTable {
    /// 以语言与状态构建列定义与数据源。
    pub(crate) fn new(lang: Lang, state: DataState) -> Self {
        let dict = Dict::of(lang);
        Self {
            lang,
            state,
            columns: vec![
                // 列宽是几何值而非间距 token：数据列需要与内容无关、可拖拽调整
                // 的稳定宽度（rem 缩放由组件内部处理）。
                Column::new("name", dict.column_name)
                    .width(px(240.))
                    .resizable(true),
                Column::new("path", dict.column_path)
                    .width(px(430.))
                    .min_width(px(200.))
                    .resizable(true),
                Column::new("pid", dict.column_pid).width(px(80.)),
                Column::new("port", dict.column_port).width(px(80.)),
            ],
        }
    }

    /// 当前状态对应的行数：只有正常态提供合成数据。
    fn rows_count(&self) -> usize {
        if self.state == DataState::Ready {
            data::ROWS.len()
        } else {
            0
        }
    }
}

impl TableDelegate for GalleryTable {
    fn columns_count(&self, _: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows_count()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        self.columns
            .get(col_ix)
            .cloned()
            .unwrap_or_else(|| Column::new("unknown", "…"))
    }

    /// 行元素使用稳定的领域 ID，而不是行号（选择与排序时身份不变）。
    fn render_tr(
        &mut self,
        row_ix: usize,
        _: &mut Window,
        _: &mut Context<'_, TableState<Self>>,
    ) -> gpui::Stateful<gpui::Div> {
        let id = data::ROWS.get(row_ix).map_or("row-unknown", |row| row.id);
        div().id(id)
    }

    /// loading 态交给 `DataTable` 内建的加载视图（保留上下文与列结构）。
    fn loading(&self, _: &App) -> bool {
        self.state == DataState::Loading
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        _: &mut Context<'_, TableState<Self>>,
    ) -> impl IntoElement {
        let dict = Dict::of(self.lang);
        let (title, description) = self.lang.state_copy(self.state);
        StateView::new(self.state, title)
            .description(description)
            .note(dict.interaction_note)
            .into_any_element()
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<'_, TableState<Self>>,
    ) -> impl IntoElement {
        let Some(row) = data::ROWS.get(row_ix) else {
            return div().into_any_element();
        };

        match col_ix {
            0 => div()
                .text_color(cx.theme().foreground)
                .child(row.name)
                .into_any_element(),
            1 => div()
                .font_family(cx.theme().mono_font_family.clone())
                .text_color(cx.theme().muted_foreground)
                // 无空格长路径在列内省略：截断是展示策略，不改动数据。
                .overflow_hidden()
                .text_ellipsis()
                .child(row.path)
                .into_any_element(),
            2 => div().child(row.pid.to_string()).into_any_element(),
            3 => div()
                .child(if row.port == 0 {
                    "—".to_string()
                } else {
                    row.port.to_string()
                })
                .into_any_element(),
            _ => div().into_any_element(),
        }
    }
}
