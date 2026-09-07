//! Linux 文件锁采集：`FileInventory` 的 `/proc/locks` + FD readlink 实现
//! （witr `locks_linux.go` / `file_linux.go` 语义）。
//!
//! 锁记录中的路径来自持有进程的 FD readlink（按 inode 匹配）；同一 PID/path
//! 同时出现在锁记录与普通 FD 时锁记录优先，不重复输出。

use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use runquiry_core::{FileLockEntry, Inspection, LockMode, LockType, Pid, ProcessFileLocks};

use super::file_diagnostics::{FileDiagnostics, FileIoStage};
use super::process::LinuxPlatform;
use super::procfs::{ProcFs, parse_locks};

/// 锁类型原词 → [`LockType`]（未知类型保留为 [`LockType::Other`]）。
fn lock_type_of(raw: &str) -> LockType {
    match raw {
        "POSIX" => LockType::Posix,
        "FLOCK" => LockType::Flock,
        "OFDLCK" => LockType::Ofdlck,
        _ => LockType::Other,
    }
}

/// 模式原词 → [`LockMode`]；未知模式返回 `None`（记诊断并跳过，不伪造）。
fn lock_mode_of(raw: &str) -> Option<LockMode> {
    match raw {
        "READ" => Some(LockMode::Read),
        "WRITE" => Some(LockMode::Write),
        "RW" => Some(LockMode::ReadWrite),
        _ => None,
    }
}

/// 持有者进程名（comm，失败为空串——witr `lockProcessName` 同语义）。
fn process_name_of(procfs: &ProcFs, pid: u32, issues: &mut FileDiagnostics) -> String {
    match procfs.read_string(&format!("{pid}/comm")) {
        Ok(raw) => raw.trim().to_string(),
        Err(error) => {
            issues.record_io(FileIoStage::Comm, &format!("进程 {pid}"), &error);
            String::new()
        }
    }
}

impl LinuxPlatform {
    /// 进程的 FD → inode → 链接目标映射（按 inode 单独匹配——witr `statKey`
    /// 注释：/proc/locks 的 device:inode 与用户态 stat 的设备号格式不完全
    /// 一致；跨文件系统 inode 撞号在实践中可忽略）。
    fn fd_inode_map(&self, pid: u32, issues: &mut FileDiagnostics) -> HashMap<u64, PathBuf> {
        let names = match self.procfs.read_dir_names(&format!("{pid}/fd")) {
            Ok(names) => names,
            Err(error) => {
                issues.record_io(FileIoStage::FdDirectory, &format!("进程 {pid}"), &error);
                return HashMap::new();
            }
        };
        let mut map = HashMap::new();
        for name in names {
            let target = match self.procfs.read_link(&format!("{pid}/fd/{name}")) {
                Ok(target) => target,
                Err(error) => {
                    issues.record_io(
                        FileIoStage::FdLink,
                        &format!("进程 {pid} 的 fd {name}"),
                        &error,
                    );
                    continue;
                }
            };
            if let Ok(metadata) = fs::metadata(&target) {
                map.insert(metadata.ino(), target);
            }
        }
        map
    }

    /// 解析 `/proc/locks` 并解析出全部锁条目；`path_filter` 为 `Some` 时只
    /// 保留匹配查询目标的锁。锁路径优先经持有进程 FD 的 inode 匹配解析，
    /// 失败回退 dev:inode 原文（witr `resolveLockPath` 兜底语义）。
    pub(super) fn lock_entries(
        &self,
        path_filter: Option<(&Path, &Path, Option<u64>)>,
    ) -> (Vec<FileLockEntry>, FileDiagnostics) {
        let mut issues = FileDiagnostics::default();
        let raw = match self.procfs.read_string("locks") {
            Ok(raw) => raw,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return (Vec::new(), FileDiagnostics::default());
            }
            Err(error) => {
                issues.record_io(FileIoStage::LockTable, "锁表", &error);
                return (Vec::new(), issues);
            }
        };
        let mut fd_cache: HashMap<u32, HashMap<u64, PathBuf>> = HashMap::new();
        let mut process_cache: HashMap<u32, String> = HashMap::new();
        let mut entries = Vec::new();
        for row in parse_locks(&raw) {
            let Ok(pid) = Pid::new(row.pid) else {
                issues.record_invalid_pid(row.pid);
                continue;
            };
            let Some(mode) = lock_mode_of(&row.mode) else {
                issues.record_invalid_lock_mode(&row.mode);
                continue;
            };
            let holder_fds = fd_cache
                .entry(row.pid)
                .or_insert_with(|| self.fd_inode_map(row.pid, &mut issues));
            let resolved: PathBuf = holder_fds
                .get(&row.inode)
                .cloned()
                .unwrap_or_else(|| PathBuf::from(&row.dev_inode));
            if let Some((target, canonical, target_ino)) = path_filter {
                let matches =
                    target_ino == Some(row.inode) || resolved == canonical || resolved == target;
                if !matches {
                    continue;
                }
            }
            let process = process_cache
                .entry(row.pid)
                .or_insert_with(|| process_name_of(&self.procfs, row.pid, &mut issues))
                .clone();
            entries.push(FileLockEntry {
                pid,
                process,
                path: resolved,
                lock_type: lock_type_of(&row.lock_type),
                mode,
            });
        }
        (entries, issues)
    }
}

impl ProcessFileLocks for LinuxPlatform {
    /// 进程持有的全部文件锁（/proc/locks 按持有者 PID 过滤；parity §1
    /// `FileContext.LockedFiles`）。与 [`FileInventory::holders`] 共享同一
    /// 解析路径；锁路径解析失败时回退 `dev:ino` 原文并携带诊断。
    fn locks_of(&self, pid: Pid) -> Inspection<Vec<FileLockEntry>> {
        let (entries, diagnostics) = self.lock_entries(None);
        let owned: Vec<FileLockEntry> = entries
            .into_iter()
            .filter(|entry| entry.pid == pid)
            .collect();
        let issues = diagnostics.into_issues();
        if issues.is_empty() {
            Inspection::complete(owned)
        } else {
            Inspection::partial(owned, issues)
        }
    }
}
