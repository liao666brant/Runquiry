//! Ports、Containers 与 File Locks 的稳定键详情渲染。

use std::time::UNIX_EPOCH;

use gpui::{AnyElement, App, IntoElement, ParentElement as _, Styled as _, div};
use gpui_component::{ActiveTheme as _, v_flex};
use runquiry_core::DiagnosticIssue;
use rust_i18n::t;

use super::{AppShell, render_detail::detail_line, render_issues::issue_message};
use crate::backend::ContainerInvestigation;
use crate::workspaces::SelectedWorkspaceRow;
use crate::{DataState, StateView};

impl AppShell {
    pub(super) fn render_port_detail(&self, cx: &App) -> Option<AnyElement> {
        let detail = self.data.ports.selected_detail()?;
        Some(match &detail.row {
            SelectedWorkspaceRow::Port(row) => v_flex()
                .min_w_0()
                .gap_2()
                .child(detail_line(
                    t!("column.protocol"),
                    &format!("{:?}", row.entry.protocol),
                ))
                .child(detail_line(t!("column.address"), &row.entry.address))
                .child(detail_line(t!("column.port"), &row.entry.port.to_string()))
                .child(detail_line(t!("column.state"), &row.entry.state))
                .child(detail_line(t!("column.pid"), &optional(row.entry.pid)))
                .child(detail_line(
                    t!("column.process"),
                    row.process.as_deref().unwrap_or("—"),
                ))
                .child(detail_line(t!("column.public"), &yes_no(row.public_bind)))
                .child(issue_panel(&detail.issues, cx))
                .into_any_element(),
            _ => stale(),
        })
    }

    pub(super) fn render_container_detail(&self, cx: &App) -> Option<AnyElement> {
        let detail = self.data.containers.selected_detail()?;
        Some(match &detail.row {
            SelectedWorkspaceRow::Container(row) => {
                container_fields(&row.summary, row.verified_host_pid, &detail.issues, cx)
            }
            _ => stale(),
        })
    }

    pub(super) fn render_file_detail(&self, cx: &App) -> Option<AnyElement> {
        let detail = self.data.files.selected_detail()?;
        Some(match &detail.row {
            SelectedWorkspaceRow::File(row) => {
                let (kind, mode) = row.lock.map_or_else(
                    || ("—".into(), "—".into()),
                    |lock| (format!("{:?}", lock.lock_type), format!("{:?}", lock.mode)),
                );
                v_flex()
                    .min_w_0()
                    .gap_2()
                    .child(detail_line(
                        t!("column.path"),
                        &row.path.display().to_string(),
                    ))
                    .child(detail_line(t!("column.pid"), &row.pid.to_string()))
                    .child(detail_line(t!("column.process"), &row.process))
                    .child(detail_line(t!("column.fd"), &optional(row.fd)))
                    .child(detail_line(t!("column.type"), &kind))
                    .child(detail_line(t!("column.mode"), &mode))
                    .child(issue_panel(&detail.issues, cx))
                    .into_any_element()
            }
            _ => stale(),
        })
    }

    pub(super) fn render_container_investigation(
        container: &ContainerInvestigation,
        cx: &App,
    ) -> AnyElement {
        container_fields(
            &container.summary,
            container.verified_host_pid,
            &container.issues,
            cx,
        )
    }
}

fn container_fields(
    summary: &runquiry_core::ContainerSummary,
    verified_pid: Option<runquiry_core::Pid>,
    issues: &[DiagnosticIssue],
    cx: &App,
) -> AnyElement {
    let started = summary
        .started_at
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or_else(|| "—".into(), |duration| duration.as_secs().to_string());
    v_flex()
        .min_w_0()
        .gap_2()
        .child(detail_line(t!("column.runtime"), &summary.key.runtime))
        .child(detail_line(
            t!("column.name"),
            summary.name.as_deref().unwrap_or("—"),
        ))
        .child(detail_line(t!("column.id"), &summary.key.id))
        .child(detail_line(
            t!("column.state"),
            summary.status.as_deref().unwrap_or("—"),
        ))
        .child(detail_line(
            t!("column.health"),
            summary.health.as_deref().unwrap_or("—"),
        ))
        .child(detail_line(
            t!("column.image"),
            summary.image.as_deref().unwrap_or("—"),
        ))
        .child(detail_line(t!("column.host_pid"), &optional(verified_pid)))
        .child(detail_line(t!("column.started_at"), &started))
        .child(issue_panel(issues, cx))
        .into_any_element()
}

fn issue_panel(issues: &[DiagnosticIssue], cx: &App) -> AnyElement {
    if issues.is_empty() {
        return div().into_any_element();
    }
    v_flex()
        .min_w_0()
        .gap_1()
        .p_2()
        .rounded_sm()
        .bg(cx.theme().warning.opacity(0.12))
        .child(t!("detail.partial_issues", count = issues.len()).to_string())
        .children(issues.iter().map(|issue| {
            div()
                .min_w_0()
                .text_xs()
                .whitespace_normal()
                .child(issue_message(issue))
        }))
        .into_any_element()
}

fn stale() -> AnyElement {
    StateView::new(DataState::Error, t!("detail.stale.title").to_string())
        .description(t!("detail.stale.description").to_string())
        .into_any_element()
}

fn optional<T: ToString>(value: Option<T>) -> String {
    value.map_or_else(
        || t!("value.unavailable").to_string(),
        |value| value.to_string(),
    )
}

fn yes_no(value: bool) -> String {
    if value {
        t!("value.yes").to_string()
    } else {
        t!("value.no").to_string()
    }
}
