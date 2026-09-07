use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use runquiry_core::{
    CapabilityStatus, DiagnosticCode, DiagnosticIssue, FileInventoryEntry, Generation, Inspection,
    LockMetadata, LockMode, LockType,
};

use crate::{
    DataState,
    workspaces::{FileLockMode, FileLockSort, FileLocksState, SelectedWorkspaceRow},
};

use super::pid;

fn row(path: &str, fd: Option<u32>, lock: Option<LockMetadata>) -> FileInventoryEntry {
    FileInventoryEntry {
        pid: pid(42),
        process: "editor".into(),
        path: PathBuf::from(path),
        fd,
        lock,
    }
}

#[test]
fn two_modes_show_fd_and_prefer_real_lock_for_same_target() {
    let lock = LockMetadata {
        lock_type: LockType::Flock,
        mode: LockMode::Write,
    };
    let mut state = FileLocksState::default();
    assert!(state.apply(
        Generation::first(),
        &CapabilityStatus::Supported,
        Inspection::complete(Arc::from([
            row("/tmp/shared", Some(9), None),
            row("/tmp/shared", None, Some(lock)),
            row("/tmp/open", Some(7), None),
        ])),
    ));
    assert_eq!(state.visible_indices().len(), 1);
    assert!(
        state
            .row(0)
            .is_some_and(|row| row.lock == Some(lock) && row.fd.is_none())
    );
    state.set_mode(FileLockMode::AllOpen);
    assert_eq!(state.visible_indices().len(), 2);
    assert!(
        state
            .visible_indices()
            .iter()
            .filter_map(|index| state.load.rows.get(*index))
            .any(|row| row.path.as_path() == Path::new("/tmp/open") && row.fd == Some(7))
    );
}

#[test]
fn unsupported_and_permission_are_not_empty() {
    let mut state = FileLocksState::default();
    assert!(state.apply(
        Generation::first(),
        &CapabilityStatus::Unsupported("Windows 不支持".into()),
        Inspection::complete(Arc::default()),
    ));
    assert_eq!(state.load.state, DataState::Unsupported);

    let issue = DiagnosticIssue::new(DiagnosticCode::PermissionDenied, "proc denied".into());
    assert!(state.apply(
        Generation::first(),
        &CapabilityStatus::Supported,
        Inspection::failed(vec![issue]),
    ));
    assert_eq!(state.load.state, DataState::PermissionDenied);
}

#[test]
fn sort_filter_and_stable_selection_use_pid_path() {
    let selected = crate::workspaces::FileKey {
        pid: pid(42),
        path: PathBuf::from("/tmp/zeta"),
    };
    let mut state = FileLocksState::default();
    state.selection.select(Some(selected.clone()));
    assert!(state.apply(
        Generation::first(),
        &CapabilityStatus::Supported,
        Inspection::complete(Arc::from([
            row(
                "/tmp/zeta",
                None,
                Some(LockMetadata {
                    lock_type: LockType::Posix,
                    mode: LockMode::Read,
                }),
            ),
            row(
                "/tmp/alpha",
                None,
                Some(LockMetadata {
                    lock_type: LockType::Flock,
                    mode: LockMode::Write,
                }),
            ),
        ])),
    ));
    state.set_sort(FileLockSort::Mode, true);
    state.set_filter("zeta");
    assert_eq!(state.selection.selected(), Some(&selected));
    assert!(!state.selection.is_stale());
    state.set_filter("alpha");
    assert!(!state.selection.is_stale(), "筛选隐藏不能伪装成目标消失");
    assert_eq!(state.selection.selected(), Some(&selected));
    let generation = state.load.generation;
    assert!(state.apply(
        generation,
        &CapabilityStatus::Supported,
        Inspection::complete(Arc::from([row(
            "/tmp/alpha",
            None,
            Some(LockMetadata {
                lock_type: LockType::Flock,
                mode: LockMode::Write,
            }),
        )])),
    ));
    assert!(state.selection.is_stale());
}

#[test]
fn selected_file_detail_exposes_fd_and_lock_metadata() {
    let lock = LockMetadata {
        lock_type: LockType::Ofdlck,
        mode: LockMode::ReadWrite,
    };
    let selected = row("/tmp/detail", None, Some(lock));
    let mut state = FileLocksState::default();
    state.selection.select(Some(crate::workspaces::FileKey {
        pid: selected.pid,
        path: selected.path.clone(),
    }));
    assert!(state.apply(
        Generation::first(),
        &CapabilityStatus::Supported,
        Inspection::complete(Arc::from([selected])),
    ));

    assert!(matches!(
        state.selected_detail().map(|detail| detail.row),
        Some(SelectedWorkspaceRow::File(row)) if row.fd.is_none() && row.lock == Some(lock)
    ));
}
