use std::sync::Arc;

use runquiry_core::{CapabilityStatus, DiagnosticCode, DiagnosticIssue, Generation, Inspection};

use crate::{DataState, workspaces::LoadPresentation};

#[test]
fn capability_and_inspection_keep_partial_data_distinct() {
    let mut load = LoadPresentation::default();
    let rows: Arc<[u32]> = Arc::from([1, 2]);
    let issue = DiagnosticIssue::new(DiagnosticCode::PermissionDenied, "部分 PID 不可读".into());
    assert!(load.apply(
        Generation::first(),
        &CapabilityStatus::Partial("受权限限制".into()),
        Inspection::partial(Arc::clone(&rows), vec![issue]),
    ));
    assert_eq!(load.state, DataState::Ready);
    assert!(load.is_partial());
    assert!(
        Arc::ptr_eq(&load.rows, &rows),
        "应用快照不应复制 100k 领域行"
    );
}

#[test]
fn partial_empty_snapshot_stays_empty_and_keeps_runtime_diagnostics() {
    let issue = DiagnosticIssue::new(
        DiagnosticCode::ExternalToolFailed,
        "podman unavailable".into(),
    );
    let mut load = LoadPresentation::default();

    assert!(load.apply(
        Generation::first(),
        &CapabilityStatus::Partial("部分运行时不可用".into()),
        Inspection::partial(Arc::<[u8]>::default(), vec![issue]),
    ));

    assert_eq!(load.state, DataState::Empty);
    assert!(load.is_partial());
    assert_eq!(load.issues.len(), 1);
    assert_eq!(load.capability_note.as_deref(), Some("部分运行时不可用"));
}

#[test]
fn empty_snapshot_does_not_hide_permission_or_capability_boundaries() {
    let cases = [
        (
            CapabilityStatus::Partial("部分权限不足".into()),
            Inspection::partial(
                Arc::<[u8]>::default(),
                vec![DiagnosticIssue::new(
                    DiagnosticCode::PermissionDenied,
                    "proc denied".into(),
                )],
            ),
            DataState::PermissionDenied,
        ),
        (
            CapabilityStatus::Unavailable("容器运行时不可用".into()),
            Inspection::complete(Arc::from([7_u8])),
            DataState::Unavailable,
        ),
        (
            CapabilityStatus::Supported,
            Inspection::failed(vec![DiagnosticIssue::new(
                DiagnosticCode::ExternalToolFailed,
                "collector failed".into(),
            )]),
            DataState::Error,
        ),
        (
            CapabilityStatus::Supported,
            Inspection::failed(Vec::new()),
            DataState::Error,
        ),
    ];

    for (capability, inspection, expected) in cases {
        let mut load = LoadPresentation::default();
        assert!(load.apply(Generation::first(), &capability, inspection));
        assert_eq!(load.state, expected);
    }
}

#[test]
fn permission_unsupported_empty_and_error_are_not_conflated() {
    let cases = [
        (
            CapabilityStatus::Supported,
            Inspection::complete(Arc::<[u8]>::default()),
            DataState::Empty,
        ),
        (
            CapabilityStatus::Supported,
            Inspection::failed(vec![DiagnosticIssue::new(
                DiagnosticCode::PermissionDenied,
                "denied".into(),
            )]),
            DataState::PermissionDenied,
        ),
        (
            CapabilityStatus::Unsupported("Windows 不支持".into()),
            Inspection::complete(Arc::<[u8]>::default()),
            DataState::Unsupported,
        ),
        (
            CapabilityStatus::Unavailable("工具缺失".into()),
            Inspection::failed(vec![DiagnosticIssue::new(
                DiagnosticCode::PlatformUnavailable,
                "missing".into(),
            )]),
            DataState::Unavailable,
        ),
    ];
    for (capability, inspection, expected) in cases {
        let mut load = LoadPresentation::default();
        assert!(load.apply(Generation::first(), &capability, inspection));
        assert_eq!(load.state, expected);
    }
}

#[test]
fn old_generation_cannot_replace_new_snapshot() {
    let mut load = LoadPresentation::default();
    let old = load.generation;
    let current = load.advance();
    assert!(!load.apply(
        old,
        &CapabilityStatus::Supported,
        Inspection::complete(Arc::from([7])),
    ));
    assert!(load.apply(
        current,
        &CapabilityStatus::Supported,
        Inspection::complete(Arc::from([9])),
    ));
    assert_eq!(&*load.rows, &[9]);
}
