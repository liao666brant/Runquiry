//! gallery 的覆盖层：Sheet、AlertDialog 与 Notification 的打开逻辑。
//!
//! 从 `main.rs` 原样搬入（A4 拆分），行为不变；仅 Ready 态通知补了一句正文。
//!
//! 本模块是 example 内部的私有模块：条目用 `pub(crate)` 暴露给 `main.rs`，
//! 这里显式豁免 `redundant_pub_crate`（否则与 `unreachable_pub` 互相冲突）。

#![allow(clippy::redundant_pub_crate)]

use gpui::{App, IntoElement, ParentElement as _, Styled as _, Window, div};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, WindowExt as _,
    button::{Button, ButtonVariant},
    dialog::DialogButtonProps,
    h_flex,
    notification::{Notification, NotificationType},
    v_flex,
};

use crate::data;
use runquiry_ui::{DataState, Dict, Lang, StateView};

/// 启动时要打开的覆盖层。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Overlay {
    /// 右侧 Sheet。
    Sheet,
    /// 确认对话框。
    Alert,
    /// 通知。
    Notification,
}

/// 打开右侧 Sheet：正常态展示详情摘要，其余状态用统一状态组件呈现。
pub(crate) fn open_sheet_overlay(
    (lang, state): (Lang, DataState),
    window: &mut Window,
    cx: &mut App,
) {
    let dict = Dict::of(lang);
    let (title, description) = lang.state_copy(state);

    window.open_sheet(cx, move |sheet, _, cx| {
        let body = if state == DataState::Ready {
            // 正常态有数据：详情面板展示对象本身，不使用状态组件。
            sheet_detail(&dict, cx).into_any_element()
        } else {
            StateView::new(state, title)
                .description(description)
                .note(dict.interaction_note)
                .into_any_element()
        };

        sheet
            .title(dict.sheet_title)
            .child(v_flex().size_full().p_4().child(body))
            .footer(
                h_flex().justify_end().child(
                    // ID 与 tab 顺序号来自 main.rs 的控件定义：焦点行能读出
                    // `sheet-close`，而不是 `unnamed region#tab-0`。
                    Button::new(crate::SHEET_CLOSE.id)
                        .small()
                        .outline()
                        .label(dict.close)
                        .tab_index(crate::SHEET_CLOSE.tab)
                        .on_click(|_, window, cx| window.close_sheet(cx)),
                ),
            )
    });
}

/// 正常态的 Sheet 详情摘要（合成数据的第一行）。
fn sheet_detail(dict: &Dict, cx: &App) -> impl IntoElement {
    let row = &data::ROWS[0];

    v_flex()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().foreground)
                .child(row.name),
        )
        .child(
            div()
                .font_family(cx.theme().mono_font_family.clone())
                .text_xs()
                .text_color(cx.theme().foreground)
                .child(row.path),
        )
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().foreground)
                .child(format!("{}: {}", dict.column_pid, row.pid)),
        )
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().foreground)
                .child(format!(
                    "{}: {}",
                    dict.column_port,
                    if row.port == 0 {
                        "—".to_string()
                    } else {
                        row.port.to_string()
                    }
                )),
        )
}

/// 打开确认对话框：命名对象与后果，按钮写结果动词。
pub(crate) fn open_alert_overlay(
    (lang, state): (Lang, DataState),
    window: &mut Window,
    cx: &mut App,
) {
    let dict = Dict::of(lang);
    let notice_type = notice_type(state);

    window.open_alert_dialog(cx, move |alert, _, cx| {
        alert
            .icon(Icon::new(IconName::TriangleAlert).text_color(cx.theme().warning))
            .title(dict.alert_title)
            .description(dict.alert_description)
            .button_props(
                DialogButtonProps::default()
                    .ok_variant(ButtonVariant::Danger)
                    .ok_text(dict.alert_confirm)
                    .show_cancel(true),
            )
            .on_ok(move |_, window, cx| {
                window.push_notification((notice_type, dict.alert_confirm), cx);
                true
            })
    });
}

/// 发送通知：类型随状态语义变化。
///
/// `state_copy(Ready)` 返回空文案（Ready 不使用状态组件），但通知需要一句有
/// 信息量的正文，因此 Ready 单独取 [`Dict::notice_ready_body`]，标题用状态的
/// 短名称，不再回退为 gallery 标题。
pub(crate) fn push_notice((lang, state): (Lang, DataState), window: &mut Window, cx: &mut App) {
    let dict = Dict::of(lang);
    let (title, description) = lang.state_copy(state);
    let (title, message) = if state == DataState::Ready {
        (dict.state_ready, dict.notice_ready_body)
    } else {
        (title, description)
    };

    window.push_notification(
        match notice_type(state) {
            NotificationType::Success => Notification::success(message),
            NotificationType::Info => Notification::info(message),
            NotificationType::Warning => Notification::warning(message),
            NotificationType::Error => Notification::error(message),
        }
        .title(title)
        // gallery 用于 QA 取证：不自动隐藏。
        .autohide(false),
        cx,
    );
}

/// 状态对应的通知类型：能力与权限边界是警告，不是错误。
const fn notice_type(state: DataState) -> NotificationType {
    match state {
        DataState::Ready => NotificationType::Success,
        DataState::Loading | DataState::Empty => NotificationType::Info,
        DataState::Error => NotificationType::Error,
        DataState::Unsupported | DataState::PermissionDenied => NotificationType::Warning,
    }
}
