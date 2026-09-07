use std::sync::Arc;

use runquiry_core::{
    CapabilityStatus, ContainerKey, ContainerSummary, DiagnosticCode, DiagnosticIssue, Generation,
    Inspection,
};

use crate::workspaces::{ContainerRow, ContainerSort, ContainersState, SelectedWorkspaceRow};

use super::pid;

fn row(runtime: &str, id: &str, verified: bool) -> ContainerRow {
    ContainerRow::new(
        ContainerSummary {
            key: ContainerKey {
                runtime: runtime.into(),
                id: id.into(),
            },
            name: Some(format!("{runtime}-{id}")),
            image: Some("example/image:latest".into()),
            status: Some("running".into()),
            health: None,
            host_pid: Some(pid(77)),
            started_at: None,
        },
        verified.then(|| pid(77)),
    )
}

#[test]
fn container_key_survives_sort_and_failed_verification_uses_fallback() {
    let selected = ContainerKey {
        runtime: "docker".into(),
        id: "abc".into(),
    };
    let mut state = ContainersState::default();
    state.selection.select(Some(selected.clone()));
    assert!(state.apply(
        Generation::first(),
        &CapabilityStatus::Supported,
        Inspection::complete(Arc::from([
            row("podman", "xyz", true),
            row("docker", "abc", false),
        ])),
    ));
    state.set_sort(ContainerSort::Name, false);
    assert_eq!(state.selection.selected(), Some(&selected));
    assert!(
        state
            .visible_indices()
            .iter()
            .filter_map(|index| state.load.rows.get(*index))
            .find(|row| row.key() == &selected)
            .is_some_and(ContainerRow::uses_fallback)
    );
}

#[test]
fn runtime_partial_keeps_rows_and_filter() {
    let issue = DiagnosticIssue::new(DiagnosticCode::ExternalToolFailed, "podman failed".into());
    let mut state = ContainersState::default();
    assert!(state.apply(
        Generation::first(),
        &CapabilityStatus::Partial("部分运行时失败".into()),
        Inspection::partial(Arc::from([row("docker", "abc", true)]), vec![issue]),
    ));
    assert!(state.load.is_partial());
    state.set_filter("DOCKER");
    assert_eq!(state.visible_indices().len(), 1);
}

#[test]
fn selected_container_detail_keeps_fallback_fields_and_issues() {
    let issue = DiagnosticIssue::new(DiagnosticCode::ExternalToolFailed, "inspect failed".into());
    let selected = row("docker", "abc", false);
    let mut state = ContainersState::default();
    state.selection.select(Some(selected.key().clone()));
    assert!(state.apply(
        Generation::first(),
        &CapabilityStatus::Partial("host PID unavailable".into()),
        Inspection::partial(Arc::from([selected]), vec![issue]),
    ));

    let detail = state.selected_detail();
    assert!(matches!(
        detail.as_ref().map(|detail| &detail.row),
        Some(SelectedWorkspaceRow::Container(row)) if row.uses_fallback()
    ));
    assert_eq!(detail.map_or(0, |detail| detail.issues.len()), 1);
}
