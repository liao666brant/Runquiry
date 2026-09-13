//! `DataTable` 与 `Input` 事件订阅。

use gpui_kit::component::{
    input::{InputEvent, InputState},
    table::TableEvent,
};
use gpui_kit::{Context, Entity, Subscription, Window};

use super::{AppShell, ShellData};

pub(super) fn table_subscriptions(
    data: &ShellData,
    query: &Entity<InputState>,
    window: &Window,
    cx: &mut Context<'_, AppShell>,
) -> Vec<Subscription> {
    let mut subscriptions = vec![
        cx.subscribe_in(
            &data.process_table,
            window,
            |shell, table, event, window, cx| match event {
                TableEvent::SelectColumn(_) => {
                    // 表头点击：delegate 已写入新排序，壳层据此重排并记录会话。
                    let sort = table.read(cx).delegate().sort();
                    shell.apply_process_header_sort(sort, cx);
                }
                // 右键先选中该行（复用单击选择路径），DataTable 随后弹出其
                // 行右键菜单；菜单项作用于该选中身份。
                TableEvent::RightClickedRow(Some(row)) => {
                    shell.on_process_table(&TableEvent::SelectRow(*row), window, cx);
                }
                _ => shell.on_process_table(event, window, cx),
            },
        ),
        cx.subscribe_in(
            &data.ports_table,
            window,
            |shell, table, event, window, cx| {
                if let TableEvent::SelectRow(row) | TableEvent::DoubleClickedRow(row) = event {
                    let key = table.read(cx).delegate().state().key_at(*row);
                    shell.select_port(key, window, cx);
                } else if let TableEvent::SelectColumn(column) = event {
                    shell.record_table_sort(*column);
                }
            },
        ),
        cx.subscribe_in(
            &data.containers_table,
            window,
            |shell, table, event, window, cx| {
                if let TableEvent::SelectRow(row) | TableEvent::DoubleClickedRow(row) = event {
                    let key = table.read(cx).delegate().state().key_at(*row);
                    shell.select_container(key, window, cx);
                } else if let TableEvent::SelectColumn(column) = event {
                    shell.record_table_sort(*column);
                }
            },
        ),
        cx.subscribe_in(
            &data.files_table,
            window,
            |shell, table, event, window, cx| {
                if let TableEvent::SelectRow(row) | TableEvent::DoubleClickedRow(row) = event {
                    let key = table.read(cx).delegate().state().key_at(*row);
                    shell.select_file(key, window, cx);
                } else if let TableEvent::SelectColumn(column) = event {
                    shell.record_table_sort(*column);
                }
            },
        ),
    ];
    subscriptions.push(
        cx.subscribe_in(query, window, |shell, input, event, window, cx| {
            match event {
                // 单输入框双职责：输入即筛选当前表格，回车发起调查。
                InputEvent::Change => {
                    shell.apply_filter(input.read(cx).value().to_string(), cx);
                }
                InputEvent::PressEnter { .. } => shell.submit_query(window, cx),
                _ => {}
            }
        }),
    );
    subscriptions
}
