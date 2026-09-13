//! 进程动作区的能力态、控件与错误呈现。

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Sizable as _, StyledExt as _, button::Button, h_flex,
    input::Input, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{AnyElement, App, Entity, IntoElement, ParentElement as _, Styled as _, div};
use runquiry_core::{CapabilityStatus, InspectError, ProcessAction, Renice};
use rust_i18n::t;

use super::AppShell;

impl AppShell {
    pub(super) fn render_process_actions(&self, shell: &Entity<Self>, cx: &App) -> AnyElement {
        let reason = self.process_action_capability.reason().map(str::to_owned);
        if matches!(
            self.process_action_capability,
            CapabilityStatus::Unsupported(_) | CapabilityStatus::Unavailable(_)
        ) {
            return v_flex()
                .mt_2()
                .gap_1()
                .p_2()
                .rounded_sm()
                .bg(cx.theme().muted)
                .child(t!("actions.unavailable").to_string())
                .when_some(reason, |view, reason| {
                    view.child(div().text_xs().whitespace_normal().child(reason))
                })
                .into_any_element();
        }
        v_flex()
            .mt_2()
            .gap_2()
            .p_2()
            .rounded_sm()
            .bg(cx.theme().muted)
            .child(div().font_semibold().child(t!("actions.title").to_string()))
            .when(self.process_action_menu_open, |view| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().primary)
                        .child(t!("actions.keyboard_menu").to_string()),
                )
            })
            .when_some(reason, |view, reason| {
                view.child(div().text_xs().whitespace_normal().child(reason))
            })
            .children(self.action_controls(shell))
            .when(self.process_action_flow.is_confirming(), |view| {
                view.child(div().text_xs().child(t!("actions.confirming").to_string()))
            })
            .when(self.process_action_flow.is_executing(), |view| {
                view.child(div().text_xs().child(t!("actions.executing").to_string()))
            })
            .when_some(self.process_action_flow.last_error(), |view, error| {
                view.child(
                    div()
                        .text_xs()
                        .whitespace_normal()
                        .text_color(cx.theme().danger)
                        .child(process_action_error(error)),
                )
            })
            .into_any_element()
    }

    fn action_controls(&self, shell: &Entity<Self>) -> [AnyElement; 2] {
        let busy = self.process_action_flow.is_busy();
        // 逐动作门禁：平台不支持的动作不渲染入口（如 Windows 的
        // 暂停/恢复/renice），并以「关闭进程树」替换被隐藏的空位。
        let supported = |action: ProcessAction| self.action_capability(action).is_usable();
        let action_button = |id: &'static str, label: String, action: ProcessAction| {
            let shell = shell.clone();
            Button::new(id)
                .small()
                .outline()
                .disabled(busy)
                .label(label)
                .on_click(move |_, window, cx| {
                    shell.update(cx, |shell, cx| {
                        shell.request_process_action(action, window, cx);
                    });
                })
        };
        let buttons: Vec<_> = [
            (
                ProcessAction::Terminate,
                "process-terminate",
                t!("actions.terminate").to_string(),
            ),
            (
                ProcessAction::Kill,
                "process-kill",
                t!("actions.kill").to_string(),
            ),
            (
                ProcessAction::KillTree,
                "process-kill-tree",
                t!("actions.kill_tree").to_string(),
            ),
            (
                ProcessAction::Pause,
                "process-pause",
                t!("actions.pause").to_string(),
            ),
            (
                ProcessAction::Resume,
                "process-resume",
                t!("actions.resume").to_string(),
            ),
        ]
        .into_iter()
        .filter(|(action, _, _)| supported(*action))
        .map(|(action, id, label)| action_button(id, label, action))
        .collect();
        let signals = h_flex()
            .flex_wrap()
            .gap_2()
            .children(buttons)
            .into_any_element();
        let renice_supported =
            Renice::try_from(0).is_ok_and(|value| supported(ProcessAction::Renice(value)));
        let renice = h_flex()
            .gap_2()
            .when(renice_supported, |row| {
                row.child(Input::new(&self.renice_input).small().w_20())
                    .child({
                        let shell = shell.clone();
                        Button::new("process-renice")
                            .small()
                            .outline()
                            .disabled(busy)
                            .label(t!("actions.renice_button").to_string())
                            .on_click(move |_, window, cx| {
                                shell.update(cx, |shell, cx| {
                                    shell.request_renice_action(window, cx);
                                });
                            })
                    })
            })
            .into_any_element();
        [signals, renice]
    }
}

pub(super) fn action_label(action: ProcessAction) -> String {
    match action {
        ProcessAction::Terminate => t!("actions.terminate").to_string(),
        ProcessAction::Kill => t!("actions.kill").to_string(),
        ProcessAction::KillTree => t!("actions.kill_tree").to_string(),
        ProcessAction::Pause => t!("actions.pause").to_string(),
        ProcessAction::Resume => t!("actions.resume").to_string(),
        ProcessAction::Renice(value) => t!("actions.renice", value = value.get()).to_string(),
    }
}

fn process_action_error(error: &InspectError) -> String {
    match error {
        InspectError::PermissionDenied { subject } => {
            t!("actions.error.permission", subject = subject).to_string()
        }
        InspectError::ProcessChanged { identity } => {
            t!("actions.error.process_changed", pid = identity.pid()).to_string()
        }
        InspectError::InvalidTarget { reason } => reason.clone(),
        InspectError::Unsupported { reason } => {
            t!("actions.error.unsupported", reason = reason).to_string()
        }
        InspectError::ExternalTool { program, detail } => t!(
            "actions.error.external_tool",
            program = program,
            detail = detail
        )
        .to_string(),
        InspectError::NotFound { .. }
        | InspectError::Ambiguous { .. }
        | InspectError::SocketOwnerUnknown { .. } => {
            t!("actions.error.generic", detail = error.to_string()).to_string()
        }
    }
}
