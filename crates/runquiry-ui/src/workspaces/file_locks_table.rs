//! File Locks 的 gpui-component `DataTable` 虚拟化适配器。

use gpui::{
    App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Stateful, Styled as _, Window, div,
};
use gpui_component::{
    ActiveTheme as _, Sizable as _, Size,
    table::{Column, ColumnSort, DataTable, TableDelegate, TableState},
};

use super::{
    FileLockSort, FileLocksState,
    file_locks::{lock_mode, lock_type},
};
use rust_i18n::t;

/// File Locks 虚拟表 delegate。
pub struct FileLocksTableDelegate {
    state: FileLocksState,
    columns: [Column; 6],
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
    /// 创建 delegate。
    pub fn new(state: FileLocksState) -> Self {
        Self {
            state,
            columns: [
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
            ],
        }
    }

    /// 替换纯状态。
    pub fn replace(&mut self, state: FileLocksState) {
        self.state = state;
    }

    /// 当前纯状态。
    pub const fn state(&self) -> &FileLocksState {
        &self.state
    }

    /// 按当前 locale 重建列标题。
    pub fn relocalize(&mut self) {
        *self = Self::new(self.state.clone());
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
        let text = match col_ix {
            0 => row.path.to_string_lossy().into_owned(),
            1 => lock_type(row.lock).into(),
            2 => lock_mode(row.lock).into(),
            3 => row.pid.to_string(),
            4 => row.process.clone(),
            5 => row.fd.map_or_else(|| "—".into(), |fd| fd.to_string()),
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
        let field = [
            FileLockSort::Path,
            FileLockSort::Type,
            FileLockSort::Mode,
            FileLockSort::Pid,
            FileLockSort::Process,
            FileLockSort::Fd,
        ]
        .get(col_ix)
        .copied()
        .unwrap_or_default();
        self.state
            .set_sort(field, !matches!(sort, ColumnSort::Descending));
        cx.emit(gpui_component::table::TableEvent::SelectColumn(col_ix));
        cx.notify();
    }
}
