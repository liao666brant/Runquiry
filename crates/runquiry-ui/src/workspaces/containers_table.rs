//! Containers 的 gpui-component `DataTable` 虚拟化适配器。

use gpui_kit::component::{
    ActiveTheme as _, Sizable as _, Size,
    table::{Column, ColumnSort, DataTable, TableDelegate, TableState},
};
use gpui_kit::{
    App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Stateful, Styled as _, Window, div,
};

use super::{ContainerSort, ContainersState};
use crate::format::{UNAVAILABLE, format_optional, format_timestamp};
use crate::workspaces::ColumnVisibility;
use rust_i18n::t;

/// Containers 虚拟表 delegate。
pub struct ContainersTableDelegate {
    state: ContainersState,
    columns: [Column; 8],
    visibility: ColumnVisibility,
}

impl std::fmt::Debug for ContainersTableDelegate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ContainersTableDelegate")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl ContainersTableDelegate {
    /// 全量列定义（含被隐藏的列）；列设置弹层按此渲染选项。
    pub fn column_defs() -> [Column; 8] {
        [
            Column::new("runtime", t!("column.runtime").to_string()).sortable(),
            Column::new("name", t!("column.name").to_string()).sortable(),
            Column::new("id", t!("column.id").to_string()).sortable(),
            Column::new("status", t!("column.state").to_string()).sortable(),
            Column::new("health", t!("column.health").to_string()).sortable(),
            Column::new("image", t!("column.image").to_string()).sortable(),
            Column::new("host-pid", t!("column.host_pid").to_string())
                .sortable()
                .text_right(),
            Column::new("started-at", t!("column.started_at").to_string()).sortable(),
        ]
    }

    /// 创建 delegate。
    pub fn new(state: ContainersState) -> Self {
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
    pub fn replace(&mut self, state: ContainersState) {
        self.state = state;
    }

    /// 当前纯状态。
    pub const fn state(&self) -> &ContainersState {
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
pub fn new_containers_table(
    state: ContainersState,
    window: &mut Window,
    cx: &mut App,
) -> Entity<TableState<ContainersTableDelegate>> {
    cx.new(|cx| {
        TableState::new(ContainersTableDelegate::new(state), window, cx)
            .row_selectable(true)
            .col_selectable(false)
            .cell_selectable(false)
            .loop_selection(false)
    })
}

/// 以 compact、条纹、双向滚动的虚拟表渲染 Containers 主数据区。
pub fn containers_table_view(
    table: &Entity<TableState<ContainersTableDelegate>>,
) -> DataTable<ContainersTableDelegate> {
    DataTable::new(table)
        .with_size(Size::Small)
        .stripe(true)
        .bordered(true)
        .scrollbar_visible(true, true)
}

/// 用新状态原位刷新 table entity。
pub fn update_containers_table(
    table: &Entity<TableState<ContainersTableDelegate>>,
    state: ContainersState,
    cx: &mut App,
) {
    table.update(cx, |table, cx| {
        table.delegate_mut().replace(state);
        table.refresh(cx);
    });
}

impl TableDelegate for ContainersTableDelegate {
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
            || format!("container-missing-{row_ix}"),
            |row| format!("container-{}", row.key().dedup_key()),
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
            "runtime" => row.summary.key.runtime.clone(),
            "name" => format_optional(row.summary.name.as_deref()),
            "id" => row.summary.key.id.clone(),
            "status" => format_optional(row.summary.status.as_deref()),
            "health" => format_optional(row.summary.health.as_deref()),
            "image" => format_optional(row.summary.image.as_deref()),
            "host-pid" => row
                .verified_host_pid
                .map_or_else(|| UNAVAILABLE.to_owned(), |pid| pid.to_string()),
            "started-at" => format_timestamp(row.summary.started_at),
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
            "name" => ContainerSort::Name,
            "status" => ContainerSort::Status,
            "health" => ContainerSort::Health,
            "image" => ContainerSort::Image,
            "host-pid" => ContainerSort::HostPid,
            "started-at" => ContainerSort::StartedAt,
            _ => ContainerSort::Key,
        };
        self.state
            .set_sort(field, !matches!(sort, ColumnSort::Descending));
        cx.emit(gpui_kit::component::table::TableEvent::SelectColumn(col_ix));
        cx.notify();
    }
}
