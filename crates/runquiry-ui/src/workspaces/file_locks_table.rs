//! File Locks 的 gpui-component `DataTable` 虚拟化适配器。

use gpui_kit::component::{
    ActiveTheme as _, Sizable as _, Size,
    table::{Column, ColumnSort, DataTable, TableDelegate, TableState},
};
use gpui_kit::{
    App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Stateful, Styled as _, Window, div,
};

use super::{
    FileLockSort, FileLocksState,
    file_locks::{lock_mode, lock_type},
};
use crate::workspaces::ColumnVisibility;
use rust_i18n::t;

/// File Locks 虚拟表 delegate。
pub struct FileLocksTableDelegate {
    state: FileLocksState,
    columns: [Column; 6],
    visibility: ColumnVisibility,
}

impl std::fmt::Debug for FileLocksTableDelegate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FileLocksTableDelegate")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl FileLocksTableDelegate {
    /// 全量列定义（含被隐藏的列）；列设置弹层按此渲染选项。
    pub fn column_defs() -> [Column; 6] {
        [
            Column::new("path", t!("column.path").to_string()).sortable(),
            Column::new("type", t!("column.type").to_string()).sortable(),
            Column::new("mode", t!("column.mode").to_string()).sortable(),
            Column::new("pid", t!("column.pid").to_string())
                .sortable()
                .text_right(),
            Column::new("process", t!("column.process").to_string()).sortable(),
            Column::new("fd", t!("column.fd").to_string())
                .sortable()
                .text_right(),
        ]
    }

    /// 创建 delegate。
    pub fn new(state: FileLocksState) -> Self {
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
    pub fn replace(&mut self, state: FileLocksState) {
        self.state = state;
    }

    /// 当前纯状态。
    pub const fn state(&self) -> &FileLocksState {
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
pub fn new_file_locks_table(
    state: FileLocksState,
    window: &mut Window,
    cx: &mut App,
) -> Entity<TableState<FileLocksTableDelegate>> {
    cx.new(|cx| {
        TableState::new(FileLocksTableDelegate::new(state), window, cx)
            .row_selectable(true)
            .col_selectable(false)
            .cell_selectable(false)
            .loop_selection(false)
    })
}

/// 以 compact、条纹、双向滚动的虚拟表渲染 File Locks 主数据区。
pub fn file_locks_table_view(
    table: &Entity<TableState<FileLocksTableDelegate>>,
) -> DataTable<FileLocksTableDelegate> {
    DataTable::new(table)
        .with_size(Size::Small)
        .stripe(true)
        .bordered(true)
        .scrollbar_visible(true, true)
}

/// 用新状态原位刷新 table entity。
pub fn update_file_locks_table(
    table: &Entity<TableState<FileLocksTableDelegate>>,
    state: FileLocksState,
    cx: &mut App,
) {
    table.update(cx, |table, cx| {
        table.delegate_mut().replace(state);
        table.refresh(cx);
    });
}

impl TableDelegate for FileLocksTableDelegate {
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
            || format!("file-missing-{row_ix}"),
            |row| format!("file-{}-{}", row.pid, row.path.to_string_lossy()),
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
            "path" => row.path.to_string_lossy().into_owned(),
            "type" => lock_type(row.lock).into(),
            "mode" => lock_mode(row.lock).into(),
            "pid" => row.pid.to_string(),
            "process" => row.process.clone(),
            "fd" => row.fd.map_or_else(|| "—".into(), |fd| fd.to_string()),
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
            "path" => FileLockSort::Path,
            "type" => FileLockSort::Type,
            "mode" => FileLockSort::Mode,
            "pid" => FileLockSort::Pid,
            "process" => FileLockSort::Process,
            "fd" => FileLockSort::Fd,
            _ => FileLockSort::Path,
        };
        self.state
            .set_sort(field, !matches!(sort, ColumnSort::Descending));
        cx.emit(gpui_kit::component::table::TableEvent::SelectColumn(col_ix));
        cx.notify();
    }
}
