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
    filter: &Entity<InputState>,
    window: &Window,
    cx: &mut Context<'_, AppShell>,
) -> Vec<Subscription> {
    let mut subscriptions = vec![
        cx.subscribe_in(
            &data.process_table,
            window,
            |shell, _, event, window, cx| shell.on_process_table(event, window, cx),
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
        cx.subscribe_in(query, window, |shell, _, event, window, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                shell.submit_query(window, cx);
            }
        }),
    );
    subscriptions.push(
        cx.subscribe_in(filter, window, |shell, input, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                shell.apply_filter(input.read(cx).value().to_string(), cx);
            }
        }),
    );
    subscriptions
}
