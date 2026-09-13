//! Ports 的 gpui-component `DataTable` 虚拟化适配器。

use gpui_kit::component::{
    ActiveTheme as _, Sizable as _, Size,
    table::{Column, ColumnSort, DataTable, TableDelegate, TableState},
};
use gpui_kit::{
    App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Stateful, Styled as _, Window, div,
};

use super::{PortSort, PortsState, ports::protocol_name};
use crate::workspaces::ColumnVisibility;
use rust_i18n::t;

/// Ports 的虚拟表 delegate；领域 ID 与 `DataTable` 行索引解耦。
pub struct PortsTableDelegate {
    state: PortsState,
    columns: [Column; 7],
    visibility: ColumnVisibility,
}

impl std::fmt::Debug for PortsTableDelegate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PortsTableDelegate")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl PortsTableDelegate {
    /// 全量列定义（含被隐藏的列）；列设置弹层按此渲染选项。
    pub fn column_defs() -> [Column; 7] {
        [
            Column::new("protocol", t!("column.protocol").to_string()).sortable(),
            Column::new("address", t!("column.address").to_string()).sortable(),
            Column::new("port", t!("column.port").to_string())
                .sortable()
                .text_right(),
            Column::new("state", t!("column.state").to_string()).sortable(),
            Column::new("pid", t!("column.pid").to_string())
                .sortable()
                .text_right(),
            Column::new("process", t!("column.process").to_string()).sortable(),
            Column::new("public-bind", t!("column.public").to_string()).sortable(),
        ]
    }

    /// 创建 delegate。
    pub fn new(state: PortsState) -> Self {
        Self {
            state,
            columns: Self::column_defs(),
            visibility: ColumnVisibility::new(),
        }
    }

    /// 整体替换隐藏列集合（来自壳层的持久化状态）。
    pub fn set_hidden(&mut self, hidden: std::collections::BTreeSet<String>) {
        self.visibility.set_hidden(hidden);
    }

    /// 替换纯状态。
    pub fn replace(&mut self, state: PortsState) {
        self.state = state;
    }

    /// 当前纯状态。
    pub const fn state(&self) -> &PortsState {
        &self.state
    }

    /// 按当前 locale 重建列标题，保留列显隐状态。
    pub fn relocalize(&mut self) {
        let hidden = self.visibility.hidden().clone();
        *self = Self::new(self.state.clone());
        self.visibility.set_hidden(hidden);
    }

    fn visible_columns(&self) -> Vec<Column> {
        self.visibility.visible(&self.columns).cloned().collect()
    }
}

/// 构造启用真实虚拟滚动的 `DataTable` 状态。
pub fn new_ports_table(
    state: PortsState,
    window: &mut Window,
    cx: &mut App,
) -> Entity<TableState<PortsTableDelegate>> {
    cx.new(|cx| {
        TableState::new(PortsTableDelegate::new(state), window, cx)
            .row_selectable(true)
            .col_selectable(false)
            .cell_selectable(false)
            .loop_selection(false)
    })
}

/// 以 compact、条纹、双向滚动的虚拟表渲染 Ports 页面主数据区。
pub fn ports_table_view(
    table: &Entity<TableState<PortsTableDelegate>>,
) -> DataTable<PortsTableDelegate> {
    DataTable::new(table)
        .with_size(Size::Small)
        .stripe(true)
        .bordered(true)
        .scrollbar_visible(true, true)
}

/// 用新状态原位刷新 table entity，供 shell 的 generation-bound 回调调用。
pub fn update_ports_table(
    table: &Entity<TableState<PortsTableDelegate>>,
    state: PortsState,
    cx: &mut App,
) {
    table.update(cx, |table, cx| {
        table.delegate_mut().replace(state);
        table.refresh(cx);
    });
}

impl TableDelegate for PortsTableDelegate {
    fn columns_count(&self, _: &App) -> usize {
        self.visibility.visible_count(&self.columns)
    }

    fn rows_count(&self, _: &App) -> usize {
        self.state.visible_indices().len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        self.visible_columns()
            .into_iter()
            .nth(col_ix)
            .unwrap_or_else(|| Column::new("unknown", "—"))
    }

    fn render_tr(
        &mut self,
        row_ix: usize,
        _: &mut Window,
        _: &mut Context<'_, TableState<Self>>,
    ) -> Stateful<gpui_kit::Div> {
        let id = self.state.row(row_ix).map_or_else(
            || format!("port-missing-{row_ix}"),
            |row| {
                let key = row.key();
                format!(
                    "port-{}-{}-{}-{}-{}",
                    protocol_name(key.protocol),
                    key.address,
                    key.port,
                    key.state,
                    key.pid
                        .map_or_else(|| "ownerless".into(), |pid| pid.to_string())
                )
            },
        );
        div().id(id)
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<'_, TableState<Self>>,
    ) -> impl IntoElement {
        let Some(row) = self.state.row(row_ix) else {
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
        let text = match key.as_ref() {
            "protocol" => protocol_name(row.entry.protocol).to_owned(),
            "address" => row.entry.address.clone(),
            "port" => row.entry.port.to_string(),
            "state" => row.entry.state.clone(),
            "pid" => row
                .entry
                .pid
                .map_or_else(|| "—".into(), |pid| pid.to_string()),
            "process" => row
                .process
                .clone()
                .unwrap_or_else(|| t!("value.unknown_owner").to_string()),
            "public-bind" => if row.public_bind {
                t!("value.yes")
            } else {
                t!("value.no")
            }
            .to_string(),
            _ => String::new(),
        };
        div()
            .truncate()
            .text_xs()
            .text_color(cx.theme().foreground)
            .child(text)
            .into_any_element()
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
        let field = match key.as_str() {
            "protocol" => PortSort::Protocol,
            "address" => PortSort::Address,
            "port" => PortSort::Port,
            "state" => PortSort::State,
            "pid" => PortSort::Pid,
            "process" => PortSort::Process,
            "public-bind" => PortSort::PublicBind,
            _ => PortSort::default(),
        };
        self.state
            .set_sort(field, !matches!(sort, ColumnSort::Descending));
        cx.emit(gpui_kit::component::table::TableEvent::SelectColumn(col_ix));
        cx.notify();
    }
}
