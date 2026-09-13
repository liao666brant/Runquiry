//! 调查结果与工作区选择详情。

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Sizable as _, StyledExt as _, button::Button, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, div,
};
use runquiry_core::{Analysis, DiagnosticIssue};
use rust_i18n::t;

use super::{AppShell, DETAIL_RATIO, render_issues::issue_message};
use crate::backend::InvestigationTarget;
use crate::processes::QueryOutcome;
use crate::session::WorkspaceId;
use crate::{DataState, StateView};

impl AppShell {
    pub(super) fn render_detail_area(&self, cx: &Context<'_, Self>) -> impl IntoElement {
        div()
            .id("detail-panel")
            .track_focus(&self.detail_focus)
            .flex_1()
            .flex_grow(DETAIL_RATIO)
            .h_full()
            .min_w_0()
            .min_h_0()
            .p_3()
            .overflow_x_hidden()
            .overflow_y_scroll()
            .child(self.detail_content(&cx.entity(), cx))
    }

    pub(super) fn detail_content(&self, shell: &Entity<Self>, cx: &App) -> AnyElement {
        if let Some(error) = &self.query_error {
            return StateView::new(DataState::Error, t!("query.failed").to_string())
                .description(error.to_string())
                .into_any_element();
        }
        if let Some(QueryOutcome::Ambiguous(candidates)) = &self.query_outcome {
            return v_flex()
                .min_w_0()
                .gap_2()
                .child(t!("query.ambiguous", count = candidates.len()).to_string())
                .children(
                    candidates
                        .iter()
                        .cloned()
                        .enumerate()
                        .map(|(index, target)| {
                            let label = target_label(&target);
                            let shell = shell.clone();
                            Button::new(format!("candidate-{index}"))
                                .small()
                                .outline()
                                .label(label)
                                .on_click(move |_, _, cx| {
                                    shell.update(cx, |this, cx| {
                                        this.choose_candidate(target.clone(), cx);
                                    });
                                })
                        }),
                )
                .into_any_element();
        }
        if matches!(self.query_outcome, Some(QueryOutcome::Empty)) {
            return StateView::new(DataState::Empty, t!("query.no_results.title").to_string())
                .description(t!("query.no_results.description").to_string())
                .into_any_element();
        }
        let Some(inspection) = &self.analysis else {
            if let Some(QueryOutcome::Unique(InvestigationTarget::Container(container))) =
                &self.query_outcome
            {
                return Self::render_container_investigation(container, cx);
            }
            return self.selection_or_placeholder(cx);
        };
        let Some(analysis) = inspection.data.as_ref() else {
            return StateView::new(DataState::Error, t!("query.failed").to_string())
                .description(format!(
                    "{}: {}",
                    t!("status.partial_issues"),
                    inspection.issues.len()
                ))
                .into_any_element();
        };
        self.render_analysis(analysis, &inspection.issues, shell, cx)
    }

    fn render_analysis(
        &self,
        analysis: &Analysis,
        issues: &[DiagnosticIssue],
        shell: &Entity<Self>,
        cx: &App,
    ) -> AnyElement {
        let resources = match analysis.details.as_ref() {
            Some(details) if details.cpu_percent.is_none() => t!("detail.sampling").to_string(),
            Some(details) => format!(
                "CPU {:.1}% · RSS {}",
                details.cpu_percent.unwrap_or_default(),
                details
                    .memory_rss_bytes
                    .map_or_else(|| "—".into(), |bytes| bytes.to_string())
            ),
            None => t!("detail.sampling").to_string(),
        };
        let environment = analysis.details.as_ref().map_or_else(Vec::new, |details| {
            self.data
                .processes
                .privacy()
                .environment(&details.environment)
        });
        v_flex()
            .min_w_0()
            .gap_3()
            .child(
                div()
                    .min_w_0()
                    .text_lg()
                    .font_bold()
                    .whitespace_normal()
                    .child(format!(
                        "{} · PID {}",
                        analysis.target.command,
                        analysis.target.identity.pid()
                    )),
            )
            .child(detail_line(
                t!("detail.overview"),
                &analysis.resolved_target,
            ))
            .child(detail_line(t!("detail.resources"), &resources))
            .child(Self::render_evidence(analysis, cx))
            .child(detail_line(
                t!("detail.environment"),
                &analysis
                    .details
                    .as_ref()
                    .map_or(0, |details| details.environment.len())
                    .to_string(),
            ))
            .when(!issues.is_empty(), |detail| {
                detail.child(
                    v_flex()
                        .gap_1()
                        .p_2()
                        .rounded_sm()
                        .bg(cx.theme().warning.opacity(0.12))
                        .child(t!("detail.partial_issues", count = issues.len()).to_string())
                        .children(issues.iter().take(3).map(|issue| {
                            div()
                                .min_w_0()
                                .text_xs()
                                .whitespace_normal()
                                .child(issue_message(issue))
                        })),
                )
            })
            .child(
                v_flex()
                    .gap_1()
                    .children(environment.into_iter().map(|entry| {
                        div().min_w_0().text_xs().whitespace_normal().child(format!(
                            "{}={}",
                            entry.key,
                            entry.value()
                        ))
                    })),
            )
            .child(
                Button::new("reveal-sensitive")
                    .small()
                    .outline()
                    .disabled(self.data.processes.privacy().is_revealed())
                    .label(t!("detail.reveal_sensitive").to_string())
                    .on_click({
                        let shell = shell.clone();
                        move |_, _, cx| shell.update(cx, Self::reveal_sensitive)
                    }),
            )
            .child(self.render_process_actions(shell, cx))
            .into_any_element()
    }

    fn selection_or_placeholder(&self, cx: &App) -> AnyElement {
        if self.detail_loading {
            let pid = self
                .data
                .processes
                .selected()
                .map_or_else(|| "—".into(), |identity| identity.pid().to_string());
            return v_flex()
                .gap_3()
                .child(detail_line(t!("column.pid"), &pid))
                .child(
                    StateView::new(DataState::Loading, t!("detail.loading.title").to_string())
                        .description(t!("detail.loading.description").to_string()),
                )
                .into_any_element();
        }
        if let Some(detail) = self.workspace_selection_detail(cx) {
            return detail;
        }
        StateView::new(DataState::Empty, t!("detail.placeholder.title").to_string())
            .description(t!("detail.placeholder.description").to_string())
            .into_any_element()
    }

    fn workspace_selection_detail(&self, cx: &App) -> Option<AnyElement> {
        match self.active_workspace() {
            WorkspaceId::Processes => self
                .data
                .processes
                .selected()
                .map(|identity| detail_line(t!("column.pid"), &identity.pid().to_string())),
            WorkspaceId::Ports => self.render_port_detail(cx),
            WorkspaceId::Containers => self.render_container_detail(cx),
            WorkspaceId::FileLocks => self.render_file_detail(cx),
        }
    }
}

fn target_label(target: &InvestigationTarget) -> String {
    match target {
        InvestigationTarget::Process(identity) => format!("PID {}", identity.pid()),
        InvestigationTarget::Container(container) => {
            format!(
                "{} · {}",
                container.summary.key.runtime, container.summary.key.id
            )
        }
    }
}

pub(super) fn detail_line(label: impl Into<gpui_kit::SharedString>, value: &str) -> AnyElement {
    v_flex()
        .min_w_0()
        .gap_1()
        .child(div().text_xs().child(label.into()))
        .child(
            div()
                .min_w_0()
                .text_sm()
                .whitespace_normal()
                .child(value.to_owned()),
        )
        .into_any_element()
}
