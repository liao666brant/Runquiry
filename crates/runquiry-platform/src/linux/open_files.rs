//! Linux 全量打开文件与锁清单。

use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use runquiry_core::{
    CapabilityStatus, DiagnosticCode, DiagnosticIssue, FileInventory, FileInventoryEntry,
    Inspection, LockMetadata, Pid,
};

use super::file_diagnostics::{FileDiagnostics, FileIoStage};
use super::process::LinuxPlatform;

impl LinuxPlatform {
    fn open_entries_for_pid(
        &self,
        pid: u32,
        issues: &mut FileDiagnostics,
    ) -> Vec<FileInventoryEntry> {
        let Ok(holder) = Pid::new(pid) else {
            return Vec::new();
        };
        let process = match self.procfs.read_string(&format!("{pid}/comm")) {
            Ok(raw) => raw.trim().to_string(),
            Err(error) => {
                issues.record_io(FileIoStage::Comm, &format!("进程 {pid}"), &error);
                String::new()
            }
        };
        let names = match self.procfs.read_dir_names(&format!("{pid}/fd")) {
            Ok(names) => names,
            Err(error) => {
                issues.record_io(FileIoStage::FdDirectory, &format!("进程 {pid}"), &error);
                return Vec::new();
            }
        };
        let mut entries = Vec::new();
        for name in names {
            let Ok(fd) = name.parse::<u32>() else {
                continue;
            };
            match self.procfs.read_link(&format!("{pid}/fd/{name}")) {
                Ok(path) => entries.push(FileInventoryEntry {
                    pid: holder,
                    process: process.clone(),
                    path,
                    fd: Some(fd),
                    lock: None,
                }),
                Err(error) => issues.record_io(
                    FileIoStage::FdLink,
                    &format!("进程 {pid} 的 fd {fd}"),
                    &error,
                ),
            }
        }
        entries
    }

    fn inventory(&self) -> Inspection<Vec<FileInventoryEntry>> {
        let pids = match self.procfs.list_pids() {
            Ok(pids) => pids,
            Err(error) => {
                return Inspection::failed(vec![DiagnosticIssue::new(
                    if error.kind() == std::io::ErrorKind::PermissionDenied {
                        DiagnosticCode::PermissionDenied
                    } else {
                        DiagnosticCode::Unknown
                    },
                    format!("/proc 不可枚举：{error}"),
                )]);
            }
        };
        let (locks, mut diagnostics) = self.lock_entries(None);
        let locked: HashSet<(u32, PathBuf)> = locks
            .iter()
            .map(|entry| (entry.pid.get(), entry.path.clone()))
            .collect();
        let mut entries: Vec<FileInventoryEntry> = locks
            .into_iter()
            .map(|entry| FileInventoryEntry {
                pid: entry.pid,
                process: entry.process,
                path: entry.path,
                fd: None,
                lock: Some(LockMetadata {
                    lock_type: entry.lock_type,
                    mode: entry.mode,
                }),
            })
            .collect();
        for pid in pids {
            entries.extend(
                self.open_entries_for_pid(pid, &mut diagnostics)
                    .into_iter()
                    .filter(|entry| !locked.contains(&(pid, entry.path.clone()))),
            );
        }
        entries.sort_by(|left, right| {
            (left.pid, &left.path, left.fd).cmp(&(right.pid, &right.path, right.fd))
        });
        let issues = diagnostics.into_issues();
        if issues.is_empty() {
            Inspection::complete(entries)
        } else {
            Inspection::partial(entries, issues)
        }
    }
}

impl FileInventory for LinuxPlatform {
    fn capability(&self) -> CapabilityStatus {
        CapabilityStatus::Supported
    }

    fn list(&self) -> Inspection<Vec<FileInventoryEntry>> {
        self.inventory()
    }

    fn holders(&self, path: &Path) -> Inspection<Vec<FileInventoryEntry>> {
        let target = path.to_path_buf();
        let canonical = fs::canonicalize(path).unwrap_or_else(|_| target.clone());
        let target_inode = fs::metadata(&canonical).ok().map(|metadata| metadata.ino());
        let (matching_locks, _) = self.lock_entries(Some((&target, &canonical, target_inode)));
        let matching_lock_keys: HashSet<(u32, PathBuf)> = matching_locks
            .into_iter()
            .map(|entry| (entry.pid.get(), entry.path))
            .collect();
        self.inventory().map(|entries| {
            entries
                .into_iter()
                .filter(|entry| {
                    entry.path == target
                        || entry.path == canonical
                        || matching_lock_keys.contains(&(entry.pid.get(), entry.path.clone()))
                })
                .collect()
        })
    }
}
