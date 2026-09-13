//! 进程动作菜单、确认对话框与异步执行。

use gpui_kit::component::{WindowExt as _, button::ButtonVariant, dialog::DialogButtonProps};
use gpui_kit::{AppContext as _, Context, Window};
use runquiry_core::{InspectError, ProcessAction, ProcessIdentity, ProcessSummary, Renice};
use rust_i18n::t;

use super::{AppShell, render_process_actions::action_label};
use crate::format::{UNAVAILABLE, format_optional, format_timestamp};
use crate::processes::{
    ActionCompletion, ActionRequest, ProcessActionShortcut, SuccessDisposition,
    accepts_action_shortcut,
};

impl AppShell {
    pub(crate) fn open_process_action_menu(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if window.has_focused_input(cx) {
            cx.propagate();
            return;
        }
        if self.current_action_identity().is_some()
            && self.process_action_capability.is_usable()
            && !self.process_action_flow.is_busy()
        {
            self.process_action_menu_open = true;
            cx.notify();
        } else {
            cx.propagate();
        }
    }

    pub(crate) fn close_process_action_menu(&mut self, cx: &mut Context<'_, Self>) {
        if self.process_action_menu_open {
            self.process_action_menu_open = false;
            cx.notify();
        } else {
            cx.propagate();
        }
    }

    pub(crate) fn select_process_action_shortcut(
        &mut self,
        shortcut: ProcessActionShortcut,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if !accepts_action_shortcut(self.process_action_menu_open, window.has_focused_input(cx)) {
            cx.propagate();
            return;
        }
        let action = match shortcut {
            ProcessActionShortcut::Kill => ProcessAction::Kill,
            ProcessActionShortcut::Terminate => ProcessAction::Terminate,
            ProcessActionShortcut::Pause => ProcessAction::Pause,
            ProcessActionShortcut::Resume => ProcessAction::Resume,
            ProcessActionShortcut::Renice => {
                let raw = self.renice_input.read(cx).value().to_string();
                let Ok(value) = raw.parse::<i8>() else {
                    self.report_renice_error(&raw, cx);
                    return;
                };
                let Ok(value) = Renice::try_from(value) else {
                    self.report_renice_error(&raw, cx);
                    return;
                };
                ProcessAction::Renice(value)
            }
        };
        self.request_process_action(action, window, cx);
    }

    pub(crate) fn request_renice_action(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let raw = self.renice_input.read(cx).value().to_string();
        let Ok(value) = raw.parse::<i8>() else {
            self.report_renice_error(&raw, cx);
            return;
        };
        let Ok(value) = Renice::try_from(value) else {
            self.report_renice_error(&raw, cx);
            return;
        };
        self.request_process_action(ProcessAction::Renice(value), window, cx);
    }

    pub(crate) fn request_process_action(
        &mut self,
        action: ProcessAction,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(identity) = self.current_action_identity().cloned() else {
            return;
        };
        self.request_process_action_for(&identity, action, window, cx);
    }

    /// 以显式身份发起动作确认（右键菜单使用右键所在行，避免依赖选中态的
    /// 事件次序）。
    pub(crate) fn request_process_action_for(
        &mut self,
        identity: &ProcessIdentity,
        action: ProcessAction,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        // 逐动作门禁（防御）：平台不支持的动作入口已隐藏，被误触达时静默
        // 拒绝，不进入确认流、不产生任何副作用。
        if !self.action_capability(action).is_usable() {
            return;
        }
        if !self.process_action_flow.request(
            &self.process_action_capability,
            identity.clone(),
            action,
        ) {
            return;
        }
        self.process_action_menu_open = false;
        let title = t!(
            "actions.confirm.title",
            action = action_label(action),
            pid = identity.pid()
        )
        .to_string();
        let description = confirmation_description(self.current_action_target(), identity);
        let ok_text = action_label(action);
        let shell = cx.entity();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let confirm_shell = shell.clone();
            let cancel_shell = shell.clone();
            let close_shell = shell.clone();
            let variant = match action {
                ProcessAction::Terminate | ProcessAction::Kill | ProcessAction::KillTree => {
                    ButtonVariant::Danger
                }
                ProcessAction::Pause | ProcessAction::Resume | ProcessAction::Renice(_) => {
                    ButtonVariant::Primary
                }
            };
            alert
                .title(title.clone())
                .description(description.clone())
                .button_props(
                    DialogButtonProps::default()
                        .show_cancel(true)
                        .cancel_text(t!("actions.cancel").to_string())
                        .ok_text(ok_text.clone())
                        .ok_variant(variant)
                        .on_ok(move |_, window, cx| {
                            confirm_shell.update(cx, |shell, cx| {
                                shell.confirm_process_action(window, cx);
                            });
                            true
                        })
                        .on_cancel(move |_, _, cx| {
                            cancel_shell.update(cx, |shell, cx| {
                                shell.process_action_flow.cancel_confirmation();
                                cx.notify();
                            });
                            true
                        }),
                )
                .on_close(move |_, _, cx| {
                    close_shell.update(cx, |shell, cx| {
                        shell.process_action_flow.cancel_confirmation();
                        cx.notify();
                    });
                })
        });
        cx.notify();
    }

    fn confirm_process_action(&mut self, window: &Window, cx: &mut Context<'_, Self>) {
        // 能力可能在确认期间退化（快照刷新取回新能力态）：过期确认不得绕过
        // 禁用状态，撤销请求并给出结构化原因。
        let capability = self.process_action_capability.clone();
        let Some(request) = self.process_action_flow.confirm_if_usable(&capability) else {
            cx.notify();
            return;
        };
        let backend = std::sync::Arc::clone(&self.backend);
        let identity = request.identity().clone();
        let action = request.action();
        let work =
            cx.background_spawn(async move { backend.execute_process_action(&identity, action) });
        self.process_action_task = Some(cx.spawn_in(window, async move |shell, cx| {
            let result = work.await;
            let _ = shell.update_in(cx, |shell, window, cx| {
                shell.apply_process_action_result(&request, result, window, cx);
            });
        }));
    }

    fn apply_process_action_result(
        &mut self,
        request: &ActionRequest,
        result: Result<(), InspectError>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let completion = self.process_action_flow.complete(request, result);
        match completion {
            Some(ActionCompletion::Succeeded(SuccessDisposition::ReturnToList)) => {
                self.analysis = None;
                self.query_outcome = None;
                self.query_error = None;
                self.detail_loading = false;
                self.data.processes.clear_selection();
                self.session.active_session_mut().select(None);
                if window.has_active_sheet(cx) {
                    window.close_sheet(cx);
                }
                self.force_refresh_active(cx);
            }
            Some(ActionCompletion::Succeeded(SuccessDisposition::RefreshDetail)) => {
                self.force_refresh_active(cx);
                self.refresh_action_detail(request, cx);
            }
            Some(ActionCompletion::Failed(_)) => cx.notify(),
            None => {}
        }
    }

    fn refresh_action_detail(&mut self, request: &ActionRequest, cx: &Context<'_, Self>) {
        let backend = std::sync::Arc::clone(&self.backend);
        let expected = request.identity().clone();
        let generation = request.generation();
        self.detail_loading = true;
        let work = cx.background_spawn({
            let identity = expected.clone();
            async move { backend.analyze(&identity) }
        });
        self.detail_task = Some(cx.spawn(async move |shell, cx| {
            let result = work.await;
            let _ = shell.update(cx, |shell, cx| {
                let current_matches = shell
                    .current_action_identity()
                    .is_some_and(|identity| identity.same_process(&expected));
                if shell.process_action_flow.generation() == generation && current_matches {
                    shell.detail_loading = false;
                    match result {
                        Ok(analysis) => {
                            shell.analysis = Some(analysis);
                            shell.query_error = None;
                        }
                        Err(error) => shell.query_error = Some(error),
                    }
                    cx.notify();
                }
            });
        }));
    }

    /// 当前详情的进程摘要；确认对话框的进程名与属主取自这里。
    fn current_action_target(&self) -> Option<&ProcessSummary> {
        self.analysis
            .as_ref()?
            .data
            .as_ref()
            .map(|analysis| &analysis.target)
    }

    fn current_action_identity(&self) -> Option<&ProcessIdentity> {
        self.current_action_target()
            .map(|summary| &summary.identity)
    }

    fn report_renice_error(&mut self, raw: &str, cx: &mut Context<'_, Self>) {
        self.process_action_menu_open = false;
        self.process_action_flow
            .report_error(InspectError::InvalidTarget {
                reason: t!("actions.renice.invalid", value = raw).to_string(),
            });
        cx.notify();
    }
}

/// 确认对话框的描述文案：五要素中的进程名、用户、启动时间与可执行路径。
///
/// PID 与动作由标题给出。进程名与属主只有分析结果有——身份冻结只覆盖 PID、
/// 启动时间与可执行路径，这两项可能缺失，缺失时回退占位符。
fn confirmation_description(target: Option<&ProcessSummary>, identity: &ProcessIdentity) -> String {
    t!(
        "actions.confirm.description",
        name = format_optional(target.map(|summary| summary.command.as_str())),
        user = format_optional(target.and_then(|summary| summary.user.as_deref())),
        started = format_timestamp(identity.start_time()),
        executable = identity.executable().map_or_else(
            || UNAVAILABLE.to_string(),
            |path| path.display().to_string()
        )
    )
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::confirmation_description;
    use crate::format::UNAVAILABLE;
    use runquiry_core::{HealthStatus, Pid, ProcessIdentity, ProcessSummary};
    use std::time::{Duration, SystemTime};

    /// 与 `format::tests` 同一时刻：2026-09-10 07:34:20 UTC。
    const STARTED_AT: u64 = 1_789_025_660;

    fn identity() -> ProcessIdentity {
        ProcessIdentity::new(
            Pid::new(1).unwrap_or(Pid::MIN),
            Some(SystemTime::UNIX_EPOCH + Duration::from_secs(STARTED_AT)),
            Some("/usr/bin/sleep".into()),
        )
    }

    fn summary(command: &str, user: Option<&str>) -> ProcessSummary {
        ProcessSummary {
            identity: identity(),
            parent_pid: None,
            command: command.to_owned(),
            command_line: None,
            user: user.map(str::to_owned),
            health: HealthStatus::Unknown,
            container: None,
            exe_deleted: false,
            capabilities: Vec::new(),
            cpu_time_seconds: None,
            cpu_percent: None,
            memory_rss_bytes: None,
            memory_percent: None,
        }
    }

    #[test]
    fn description_carries_all_five_elements() {
        let described =
            confirmation_description(Some(&summary("sleep", Some("syspetro"))), &identity());

        assert!(described.contains("sleep"));
        assert!(described.contains("syspetro"));
        assert!(described.contains("2026-09-10 07:34:20 UTC"));
        assert!(described.contains("/usr/bin/sleep"));
    }

    #[test]
    fn missing_analysis_falls_back_to_placeholders() {
        let described = confirmation_description(None, &identity());

        // 进程名与属主两项缺失，各回退一次占位符。
        assert_eq!(described.matches(UNAVAILABLE).count(), 2);
    }
}
