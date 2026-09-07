//! Containers 的 gpui-component `DataTable` 虚拟化适配器。

use gpui::{
    App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Stateful, Styled as _, Window, div,
};
use gpui_component::{
    ActiveTheme as _, Sizable as _, Size,
    table::{Column, ColumnSort, DataTable, TableDelegate, TableState},
};

use super::{ContainerSort, ContainersState};
use rust_i18n::t;

/// Containers 虚拟表 delegate。
pub struct ContainersTableDelegate {
    state: ContainersState,
    columns: [Column; 8],
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
    /// 创建 delegate。
    pub fn new(state: ContainersState) -> Self {
        Self {
            state,
            columns: [
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
            ],
        }
    }

    /// 替换纯状态。
    pub fn replace(&mut self, state: ContainersState) {
        self.state = state;
    }

    /// 当前纯状态。
    pub const fn state(&self) -> &ContainersState {
        &self.state
    }

    /// 按当前 locale 重建列标题。
    pub fn relocalize(&mut self) {
        *self = Self::new(self.state.clone());
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
        self.columns.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.state.visible_indices().len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        self.columns[col_ix].clone()
    }

    fn render_tr(
        &mut self,
        row_ix: usize,
        _: &mut Window,
        _: &mut Context<'_, TableState<Self>>,
    ) -> Stateful<gpui::Div> {
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
        let text = match col_ix {
            0 => row.summary.key.runtime.clone(),
            1 => optional(row.summary.name.as_deref()),
            2 => row.summary.key.id.clone(),
            3 => optional(row.summary.status.as_deref()),
            4 => optional(row.summary.health.as_deref()),
            5 => optional(row.summary.image.as_deref()),
            6 => row
                .verified_host_pid
                .map_or_else(|| "—".into(), |pid| pid.to_string()),
            7 => row
                .summary
                .started_at
                .map_or_else(|| "—".into(), |time| format!("{time:?}")),
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
        let field = match col_ix {
            1 => ContainerSort::Name,
            3 => ContainerSort::Status,
            4 => ContainerSort::Health,
            5 => ContainerSort::Image,
            6 => ContainerSort::HostPid,
            7 => ContainerSort::StartedAt,
            _ => ContainerSort::Key,
        };
        self.state
            .set_sort(field, !matches!(sort, ColumnSort::Descending));
        cx.emit(gpui_component::table::TableEvent::SelectColumn(col_ix));
        cx.notify();
    }
}

fn optional(value: Option<&str>) -> String {
    value.unwrap_or("—").to_owned()
}
