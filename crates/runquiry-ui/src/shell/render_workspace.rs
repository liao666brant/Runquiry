//! 调查栏与四工作区主数据区。

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, Styled as _, div,
};
use gpui_component::{
    ActiveTheme as _, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::Input,
    v_flex,
};
use rust_i18n::t;

use super::{AppShell, MAIN_RATIO};
use crate::processes::{ProcessTable, TargetKind};
use crate::session::WorkspaceId;
use crate::workspaces::{
    FileLockMode, PortMode, containers_table_view, file_locks_table_view, ports_table_view,
};
use crate::{DataState, StateView};

impl AppShell {
    pub(super) fn render_main_area(
        &self,
        inline: bool,
        cx: &Context<'_, Self>,
    ) -> impl IntoElement {
        v_flex()
            .id("main-panel")
            .track_focus(&self.main_focus)
            .flex_1()
            .flex_grow(MAIN_RATIO)
            .h_full()
            .min_w_0()
            .min_h_0()
            .when(inline, |panel| {
                panel.border_r_1().border_color(cx.theme().border)
            })
            .p_3()
            .gap_3()
            .child(self.render_investigation(cx))
            .child(self.render_workspace_controls(cx))
            .child(Input::new(&self.filter_input).small().cleanable(true))
            .child(self.partial_banner(cx))
            .child(div().flex_1().min_h_0().child(self.active_table()))
    }

    fn render_investigation(&self, cx: &Context<'_, Self>) -> impl IntoElement {
        let kinds = [
            TargetKind::Name,
            TargetKind::Pid,
            TargetKind::Port,
            TargetKind::File,
            TargetKind::Container,
        ];
        v_flex()
            .gap_2()
            .child(h_flex().gap_1().children(kinds.map(|kind| {
                Button::new(format!("target-{}", target_key(kind)))
                    .xsmall()
                    .ghost()
                    .selected(self.target_kind == kind)
                    .label(t!(target_label(kind)).to_string())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_target_kind(kind, cx);
                    }))
            })))
            .child(
                h_flex()
                    .min_w_0()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.query_input).small().cleanable(true)),
                    )
                    .child(
                        Button::new("investigate")
                            .small()
                            .primary()
                            .label(t!("query.submit").to_string())
                            .on_click(
                                cx.listener(|this, _, window, cx| this.submit_query(window, cx)),
                            ),
                    ),
            )
    }

    fn active_table(&self) -> AnyElement {
        match self.active_workspace() {
            WorkspaceId::Processes => {
                ProcessTable::new(&self.data.process_table).into_any_element()
            }
            WorkspaceId::Ports => workspace_table(
                self.data.ports.load.state,
                ports_table_view(&self.data.ports_table),
            ),
            WorkspaceId::Containers => workspace_table(
                self.data.containers.load.state,
                containers_table_view(&self.data.containers_table),
            ),
            WorkspaceId::FileLocks => workspace_table(
                self.data.files.load.state,
                file_locks_table_view(&self.data.files_table),
            ),
        }
    }

    fn render_workspace_controls(&self, cx: &Context<'_, Self>) -> AnyElement {
        match self.active_workspace() {
            WorkspaceId::Ports => h_flex()
                .gap_1()
                .child(
                    Button::new("ports-listening")
                        .xsmall()
                        .ghost()
                        .selected(self.data.ports.mode == PortMode::Listening)
                        .label(t!("ports.listening").to_string())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.set_port_mode(PortMode::Listening, cx);
                        })),
                )
                .child(
                    Button::new("ports-all")
                        .xsmall()
                        .ghost()
                        .selected(self.data.ports.mode == PortMode::All)
                        .label(t!("ports.all").to_string())
                        .on_click(
                            cx.listener(|this, _, _, cx| this.set_port_mode(PortMode::All, cx)),
                        ),
                )
                .into_any_element(),
            WorkspaceId::FileLocks => h_flex()
                .gap_1()
                .child(
                    Button::new("files-locked")
                        .xsmall()
                        .ghost()
                        .selected(self.data.files.mode == FileLockMode::Locked)
                        .label(t!("files.locked").to_string())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.set_file_mode(FileLockMode::Locked, cx);
                        })),
                )
                .child(
                    Button::new("files-open")
                        .xsmall()
                        .ghost()
                        .selected(self.data.files.mode == FileLockMode::AllOpen)
                        .label(t!("files.all_open").to_string())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.set_file_mode(FileLockMode::AllOpen, cx);
                        })),
                )
                .into_any_element(),
            WorkspaceId::Processes => Button::new("process-sort-pid")
                .xsmall()
                .outline()
                .label(t!("processes.sort_pid").to_string())
                .on_click(cx.listener(|this, _, _, cx| this.toggle_process_pid_sort(cx)))
                .into_any_element(),
            WorkspaceId::Containers => div().into_any_element(),
        }
    }
}

const fn target_key(kind: TargetKind) -> &'static str {
    match kind {
        TargetKind::Name => "name",
        TargetKind::Pid => "pid",
        TargetKind::Port => "port",
        TargetKind::File => "file",
        TargetKind::Container => "container",
    }
}

const fn target_label(kind: TargetKind) -> &'static str {
    match kind {
        TargetKind::Name => "query.kind.name",
        TargetKind::Pid => "query.kind.pid",
        TargetKind::Port => "query.kind.port",
        TargetKind::File => "query.kind.file",
        TargetKind::Container => "query.kind.container",
    }
}

fn workspace_table(state: DataState, table: impl IntoElement) -> AnyElement {
    if state == DataState::Ready {
        table.into_any_element()
    } else {
        let (title, description) = workspace_state_copy(state);
        StateView::new(state, title)
            .description(description)
            .into_any_element()
    }
}

fn workspace_state_copy(state: DataState) -> (String, String) {
    let suffix = match state {
        DataState::Ready => return (String::new(), String::new()),
        DataState::Loading => "loading",
        DataState::Empty => "empty",
        DataState::Error => "error",
        DataState::Unsupported => "unsupported",
        DataState::PermissionDenied => "permission_denied",
    };
    (
        t!(format!("workspace_state.{suffix}.title")).to_string(),
        t!(format!("workspace_state.{suffix}.description")).to_string(),
    )
}
