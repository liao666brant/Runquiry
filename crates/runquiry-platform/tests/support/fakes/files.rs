use std::path::Path;

use runquiry_core::{
    CapabilityStatus, FileInventory, FileInventoryEntry, Inspection, LockMetadata, LockMode,
    LockType,
};

use super::FakePlatform;

impl FileInventory for FakePlatform {
    fn capability(&self) -> CapabilityStatus {
        self.capability_for()
    }

    fn list(&self) -> Inspection<Vec<FileInventoryEntry>> {
        self.holders(Path::new("/opt/runquiry-fixtures/var/fxt-daemon.lock"))
    }

    fn holders(&self, path: &Path) -> Inspection<Vec<FileInventoryEntry>> {
        let entry = FileInventoryEntry {
            pid: self.baseline.pid(),
            process: String::from("fxt-daemon"),
            path: path.to_path_buf(),
            fd: None,
            lock: Some(LockMetadata {
                lock_type: LockType::Flock,
                mode: LockMode::Write,
            }),
        };
        self.inspect(vec![entry])
    }
}
