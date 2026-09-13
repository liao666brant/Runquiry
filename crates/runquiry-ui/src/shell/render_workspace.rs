//! 调查栏与四工作区主数据区。

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex,
    input::Input,
    popover::Popover,
    table::Column,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    Styled as _, div,
};
use rust_i18n::t;

use super::{AppShell, MAIN_RATIO};
use crate::processes::{ProcessTable, TargetKind};
use crate::session::WorkspaceId;
use crate::workspaces::{
    FileLockMode, PortMode, containers_table_view, file_locks_table_view, ports_table_view,
};
use crate::{DataState, StateView, workspace_state_copy};

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
            .when_some(self.render_workspace_controls(cx), |panel, controls| {
                panel.child(controls)
            })
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
            .child(
                h_flex()
                    .gap_1()
                    .children(kinds.map(|kind| {
                        Button::new(format!("target-{}", target_key(kind)))
                            .xsmall()
                            .ghost()
                            .selected(self.target_kind == kind)
                            .label(t!(target_label(kind)).to_string())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.set_target_kind(kind, cx);
                            }))
                    }))
                    // 列设置跟随顶部导航行，紧随「容器」之后。
                    .child(self.render_column_settings(cx)),
            )
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
                self.data.ports.load.boundary_reason.as_deref(),
                ports_table_view(&self.data.ports_table),
            ),
            WorkspaceId::Containers => workspace_table(
                self.data.containers.load.state,
                self.data.containers.load.boundary_reason.as_deref(),
                containers_table_view(&self.data.containers_table),
            ),
            WorkspaceId::FileLocks => workspace_table(
                self.data.files.load.state,
                self.data.files.load.boundary_reason.as_deref(),
                file_locks_table_view(&self.data.files_table),
            ),
        }
    }

    fn render_workspace_controls(&self, cx: &Context<'_, Self>) -> Option<AnyElement> {
        let workspace = self.active_workspace();
        let interactive = self.data.interactions_enabled(workspace);
        match workspace {
            WorkspaceId::Ports => Some(
                h_flex()
                    .gap_1()
                    .child(
                        Button::new("ports-listening")
                            .xsmall()
                            .ghost()
                            .disabled(!interactive)
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
                            .disabled(!interactive)
                            .selected(self.data.ports.mode == PortMode::All)
                            .label(t!("ports.all").to_string())
                            .on_click(
                                cx.listener(|this, _, _, cx| this.set_port_mode(PortMode::All, cx)),
                            ),
                    )
                    .into_any_element(),
            ),
            WorkspaceId::FileLocks => Some(
                h_flex()
                    .gap_1()
                    .child(
                        Button::new("files-locked")
                            .xsmall()
                            .ghost()
                            .disabled(!interactive)
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
                            .disabled(!interactive)
                            .selected(self.data.files.mode == FileLockMode::AllOpen)
                            .label(t!("files.all_open").to_string())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_file_mode(FileLockMode::AllOpen, cx);
                            })),
                    )
                    .into_any_element(),
            ),
            // 进程/容器无模式切换：排序走表头，列设置在调查栏。
            WorkspaceId::Processes | WorkspaceId::Containers => None,
        }
    }

    /// 列设置图标按钮：弹出当前工作区表格的列显隐勾选层。
    ///
    /// 勾选项在弹层每次渲染时按壳层最新状态构建（通过捕获的 shell 句柄
    /// 读取与派发），保证连续勾选不会基于过期快照。
    fn render_column_settings(&self, cx: &Context<'_, Self>) -> AnyElement {
        let workspace = self.active_workspace();
        let shell = cx.entity();
        let trigger_id = SharedString::from(format!("column-settings-{}", workspace.key()));
        Popover::new(trigger_id.clone())
            .trigger(
                Button::new(SharedString::from(format!("{trigger_id}-trigger")))
                    .xsmall()
                    .ghost()
                    .icon(IconName::Settings2),
            )
            .content(move |_, _, cx| {
                let hidden = shell.read(cx).hidden_columns(workspace);
                let items = column_defs(workspace).into_iter().map(|column| {
                    let key = column.key.to_string();
                    let checked = !hidden.contains(&key);
                    Checkbox::new(SharedString::from(format!(
                        "column-toggle-{}-{key}",
                        workspace.key()
                    )))
                    .label(column.name.to_string())
                    .checked(checked)
                    .on_click({
                        let shell = shell.clone();
                        let key = key.clone();
                        move |checked: &bool, _, app| {
                            shell.update(app, |this, cx| {
                                this.set_column_visible(workspace, &key, *checked, cx);
                            });
                        }
                    })
                });
                v_flex().gap_1().children(items).into_any_element()
            })
            .into_any_element()
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

/// 工作区表格的全量列定义（含被隐藏的列）。
fn column_defs(workspace: WorkspaceId) -> Vec<Column> {
    match workspace {
        WorkspaceId::Processes => crate::processes::ProcessTableDelegate::column_defs(),
        WorkspaceId::Ports => crate::workspaces::PortsTableDelegate::column_defs().to_vec(),
        WorkspaceId::Containers => {
            crate::workspaces::ContainersTableDelegate::column_defs().to_vec()
        }
        WorkspaceId::FileLocks => crate::workspaces::FileLocksTableDelegate::column_defs().to_vec(),
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

fn workspace_table(
    state: DataState,
    boundary_reason: Option<&str>,
    table: impl IntoElement,
) -> AnyElement {
    if state == DataState::Ready {
        table.into_any_element()
    } else {
        let (title, description) = workspace_state_copy(state);
        StateView::new(state, title)
            .description(description)
            .when_some(boundary_reason, |view, reason| {
                // 能力/环境边界的平台原因原样呈现：UI 不改写平台结论。
                view.note(reason)
            })
            .into_any_element()
    }
}
