use std::sync::Arc;

use runquiry_core::{CapabilityStatus, Generation, Inspection, OpenPortEntry, Protocol};

use crate::workspaces::{PortMode, PortRow, PortSort, PortsState, SelectedWorkspaceRow};

use super::{pid, port};

fn row(address: &str, state: &str, owner: Option<u32>, process: Option<&str>) -> PortRow {
    PortRow::new(
        OpenPortEntry {
            pid: owner.map(pid),
            port: port(if state == "LISTEN" { 8080 } else { 9000 }),
            address: address.into(),
            protocol: Protocol::Tcp,
            state: state.into(),
        },
        process.map(str::to_owned),
    )
}

#[test]
fn modes_filter_and_keep_ownerless_public_bind() {
    let rows: Arc<[PortRow]> = Arc::from([
        row("0.0.0.0", "LISTEN", None, None),
        row("127.0.0.1", "ESTAB", Some(42), Some("curl")),
    ]);
    let mut state = PortsState::default();
    assert!(state.apply(
        Generation::first(),
        &CapabilityStatus::Supported,
        Inspection::complete(rows),
    ));
    assert_eq!(state.visible_indices().len(), 1);
    let listening = state.row(0);
    assert!(listening.is_some_and(|row| row.entry.pid.is_none() && row.public_bind));

    let generation = state.set_mode(PortMode::All);
    assert_eq!(generation.value(), 1);
    assert_eq!(state.visible_indices().len(), 2);
    state.set_filter("curl");
    assert_eq!(state.visible_indices().len(), 1);
    assert!(
        state
            .row(0)
            .is_some_and(|row| row.process.as_deref() == Some("curl"))
    );
}

#[test]
fn stable_key_survives_sort_and_missing_target_becomes_stale() {
    let first = row("127.0.0.1", "LISTEN", Some(1), Some("alpha"));
    let selected = first.key();
    let mut state = PortsState::default();
    state.selection.select(Some(selected.clone()));
    assert!(state.apply(
        Generation::first(),
        &CapabilityStatus::Partial("one namespace denied".into()),
        Inspection::complete(Arc::from([first, row("::", "LISTEN", None, None)])),
    ));
    assert!(state.load.is_partial());
    state.set_sort(PortSort::Pid, false);
    assert_eq!(state.selection.selected(), Some(&selected));
    assert!(!state.selection.is_stale());
    let generation = state.load.generation;
    assert!(state.apply(
        generation,
        &CapabilityStatus::Supported,
        Inspection::complete(Arc::from([row("::", "LISTEN", None, None)])),
    ));
    assert_eq!(state.selection.selected(), Some(&selected));
    assert!(state.selection.is_stale());
}

#[test]
fn hundred_thousand_rows_remain_index_backed() {
    let rows: Arc<[PortRow]> = (1..=100_000)
        .map(|value| {
            let mut item = row("127.0.0.1", "LISTEN", Some(1), Some("server"));
            let port_number = u16::try_from(value % 65_534 + 1).unwrap_or(1);
            item.entry.port = port(port_number);
            item
        })
        .collect::<Vec<_>>()
        .into();
    let mut state = PortsState::default();
    assert!(state.apply(
        Generation::first(),
        &CapabilityStatus::Supported,
        Inspection::complete(Arc::clone(&rows)),
    ));
    assert_eq!(state.visible_indices().len(), 100_000);
    assert!(Arc::ptr_eq(&state.load.rows, &rows));
}

#[test]
fn selected_port_detail_uses_current_snapshot_and_becomes_stale() {
    let selected = row("0.0.0.0", "LISTEN", Some(42), Some("server"));
    let mut state = PortsState::default();
    state.selection.select(Some(selected.key()));
    assert!(state.apply(
        Generation::first(),
        &CapabilityStatus::Supported,
        Inspection::complete(Arc::from([selected])),
    ));

    let detail = state.selected_detail();
    assert!(matches!(
        detail.map(|detail| detail.row),
        Some(SelectedWorkspaceRow::Port(row)) if row.public_bind && row.process.as_deref() == Some("server")
    ));

    let generation = state.load.generation;
    assert!(state.apply(
        generation,
        &CapabilityStatus::Supported,
        Inspection::complete(Arc::default()),
    ));
    assert!(matches!(
        state.selected_detail().map(|detail| detail.row),
        Some(SelectedWorkspaceRow::Stale)
    ));
}
