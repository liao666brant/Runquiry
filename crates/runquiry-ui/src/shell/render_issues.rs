//! 工作区部分成功与采集诊断的呈现。

use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{AnyElement, Context, IntoElement as _, ParentElement as _, Styled as _, div};
use runquiry_core::{CapabilityStatus, DiagnosticIssue};
use rust_i18n::t;

use super::AppShell;
use crate::session::WorkspaceId;

const VISIBLE_ISSUES: usize = 3;

impl AppShell {
    pub(super) fn partial_banner(&self, cx: &Context<'_, Self>) -> AnyElement {
        let (issues, note) = self.active_diagnostics();
        let visible = issues.len().min(VISIBLE_ISSUES);
        v_flex()
            .when(!issues.is_empty() || note.is_some(), |view| {
                view.gap_1()
                    .p_2()
                    .rounded_sm()
                    .bg(cx.theme().warning.opacity(0.12))
                    .child(format!("{}: {}", t!("status.partial_issues"), issues.len()))
                    .when_some(note, |view, note| {
                        view.child(div().text_xs().child(note.to_owned()))
                    })
                    .children(
                        issues
                            .iter()
                            .take(VISIBLE_ISSUES)
                            .map(|issue| div().text_xs().child(issue_message(issue))),
                    )
                    .when(issues.len() > visible, |view| {
                        view.child(div().text_xs().child(
                            t!("status.more_issues", count = issues.len() - visible).to_string(),
                        ))
                    })
            })
            .into_any_element()
    }

    fn active_diagnostics(&self) -> (&[DiagnosticIssue], Option<&str>) {
        match self.active_workspace() {
            WorkspaceId::Processes => (
                &self.data.process_issues,
                match &self.data.process_capability {
                    CapabilityStatus::Partial(note) => Some(note.as_str()),
                    _ => None,
                },
            ),
            WorkspaceId::Ports => (
                &self.data.ports.load.issues,
                self.data.ports.load.capability_note.as_deref(),
            ),
            WorkspaceId::Containers => (
                &self.data.containers.load.issues,
                self.data.containers.load.capability_note.as_deref(),
            ),
            WorkspaceId::FileLocks => (
                &self.data.files.load.issues,
                self.data.files.load.capability_note.as_deref(),
            ),
        }
    }
}

pub(super) fn issue_message(issue: &DiagnosticIssue) -> String {
    issue.message().replace("到 PID", "到\u{a0}PID")
}
