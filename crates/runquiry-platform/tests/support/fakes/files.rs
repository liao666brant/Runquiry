use std::path::Path;

use runquiry_core::{
    CapabilityStatus, FileInventory, FileLockEntry, Inspection, LockMode, LockType,
};

use super::FakePlatform;

impl FileInventory for FakePlatform {
    fn capability(&self) -> CapabilityStatus {
        self.capability_for()
    }

    fn holders(&self, path: &Path) -> Inspection<Vec<FileLockEntry>> {
        let entry = FileLockEntry {
            pid: self.baseline.pid(),
            process: String::from("fxt-daemon"),
            path: path.to_path_buf(),
            lock_type: LockType::Flock,
            mode: LockMode::Write,
        };
        self.inspect(vec![entry])
    }
}
