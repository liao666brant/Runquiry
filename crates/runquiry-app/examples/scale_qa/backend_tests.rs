use std::sync::Arc;

use runquiry_core::{CapabilityStatus, InspectError, Pid};
use runquiry_ui::{
    WorkspaceId,
    backend::{WorkspaceBackend, WorkspaceSnapshot},
};

use super::{SCALE_ROWS, SyntheticBackend};

#[test]
fn snapshots_have_100k_stable_rows_when_each_workspace_loads() -> Result<(), InspectError> {
    // Given: a backend holding all four prebuilt synthetic snapshots.
    let backend = SyntheticBackend::new()?;

    // When: each workspace is loaded once.
    let snapshots = WorkspaceId::ALL.map(|workspace| backend.load(workspace));

    // Then: every snapshot has 100k rows and stable first/last domain identities.
    let WorkspaceSnapshot::Processes { inspection, .. } = &snapshots[0] else {
        return Err(InspectError::Unsupported {
            reason: String::from("process snapshot mismatch"),
        });
    };
    let Some(rows) = inspection.data() else {
        return Err(InspectError::Unsupported {
            reason: String::from("process rows missing"),
        });
    };
    assert_eq!(rows.len(), SCALE_ROWS);
    assert_eq!(rows[0].identity.pid().get(), 1);
    assert_eq!(rows[SCALE_ROWS - 1].identity.pid().get(), 100_000);

    let WorkspaceSnapshot::Ports { inspection, .. } = &snapshots[1] else {
        return Err(InspectError::Unsupported {
            reason: String::from("port snapshot mismatch"),
        });
    };
    let Some(rows) = inspection.data() else {
        return Err(InspectError::Unsupported {
            reason: String::from("port rows missing"),
        });
    };
    assert_eq!(rows.len(), SCALE_ROWS);
    assert_eq!(rows[0].entry.address, "10.0.0.0");
    assert_eq!(rows[SCALE_ROWS - 1].entry.pid.map(Pid::get), Some(100_000));

    let WorkspaceSnapshot::Containers { inspection, .. } = &snapshots[2] else {
        return Err(InspectError::Unsupported {
            reason: String::from("container snapshot mismatch"),
        });
    };
    let Some(rows) = inspection.data() else {
        return Err(InspectError::Unsupported {
            reason: String::from("container rows missing"),
        });
    };
    assert_eq!(rows.len(), SCALE_ROWS);
    assert_eq!(rows[0].summary.key.id, "scale-000000");
    assert_eq!(rows[SCALE_ROWS - 1].summary.key.id, "scale-099999");

    let WorkspaceSnapshot::FileLocks { inspection, .. } = &snapshots[3] else {
        return Err(InspectError::Unsupported {
            reason: String::from("file snapshot mismatch"),
        });
    };
    let Some(rows) = inspection.data() else {
        return Err(InspectError::Unsupported {
            reason: String::from("file rows missing"),
        });
    };
    assert_eq!(rows.len(), SCALE_ROWS);
    assert_eq!(
        rows[0].path,
        std::path::PathBuf::from("/synthetic/scale-qa/file-000000")
    );
    assert_eq!(rows[SCALE_ROWS - 1].pid.get(), 100_000);
    Ok(())
}

#[test]
fn load_reuses_snapshot_and_advances_visible_refresh_count_when_repeated()
-> Result<(), InspectError> {
    // Given: a backend holding one preallocated process snapshot.
    let backend = SyntheticBackend::new()?;

    // When: the Process workspace completes two refreshes.
    let first = backend.load(WorkspaceId::Processes);
    let second = backend.load(WorkspaceId::Processes);

    // Then: the Arc and identity remain stable while the visible synthetic count advances.
    let WorkspaceSnapshot::Processes {
        capability: first_capability,
        inspection: first_inspection,
    } = first
    else {
        return Err(InspectError::Unsupported {
            reason: String::from("first process snapshot mismatch"),
        });
    };
    let WorkspaceSnapshot::Processes {
        capability: second_capability,
        inspection: second_inspection,
    } = second
    else {
        return Err(InspectError::Unsupported {
            reason: String::from("second process snapshot mismatch"),
        });
    };
    let Some(first_rows) = first_inspection.data() else {
        return Err(InspectError::Unsupported {
            reason: String::from("first process rows missing"),
        });
    };
    let Some(second_rows) = second_inspection.data() else {
        return Err(InspectError::Unsupported {
            reason: String::from("second process rows missing"),
        });
    };
    assert!(Arc::ptr_eq(first_rows, second_rows));
    assert_eq!(
        first_rows[0].identity.pid().get(),
        second_rows[0].identity.pid().get()
    );
    assert_eq!(first_capability, CapabilityStatus::Supported);
    assert!(matches!(second_capability, CapabilityStatus::Partial(_)));
    assert_eq!(backend.completed_loads(), 2);
    Ok(())
}

#[test]
fn process_control_stays_unsupported_when_scale_backend_is_used() -> Result<(), InspectError> {
    // Given: the synthetic backend used only for desktop scale QA.
    let backend = SyntheticBackend::new()?;

    // When: the shell asks for process-control capability.
    let capability = backend.process_control_capability();

    // Then: the potentially destructive operations remain disabled.
    assert!(matches!(capability, CapabilityStatus::Unsupported(_)));
    Ok(())
}
