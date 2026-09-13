//! Ports、Containers 与 File Locks 工作区。

mod common;
mod containers;
mod containers_table;
mod detail;
mod file_locks;
mod file_locks_table;
mod ports;
mod ports_table;

pub use common::{ColumnVisibility, LoadPresentation, StableSelection, interactions_enabled};
pub use containers::{ContainerRow, ContainerSort, ContainersState};
pub use containers_table::{
    ContainersTableDelegate, containers_table_view, new_containers_table, update_containers_table,
};
pub use detail::{SelectedWorkspaceDetail, SelectedWorkspaceRow};
pub use file_locks::{FileKey, FileLockMode, FileLockSort, FileLocksState};
pub use file_locks_table::{
    FileLocksTableDelegate, file_locks_table_view, new_file_locks_table, update_file_locks_table,
};
pub use ports::{PortKey, PortMode, PortRow, PortSort, PortsState};
pub use ports_table::{PortsTableDelegate, new_ports_table, ports_table_view, update_ports_table};

#[cfg(test)]
mod tests;
