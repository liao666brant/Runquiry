//! Linux 文件锁采集：`FileInventory` 的 `/proc/locks` + FD readlink 实现
//! （witr `locks_linux.go` / `file_linux.go` 语义）。
//!
//! 锁记录中的路径来自持有进程的 FD readlink（按 inode 匹配）；同一 PID/path
//! 同时出现在锁记录与普通 FD 时锁记录优先，不重复输出。

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use runquiry_core::{
    CapabilityStatus, DiagnosticCode, DiagnosticIssue, FileInventory, FileLockEntry, Inspection,
    LockMode, LockType, Pid, ProcessFileLocks,
};

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
fn process_name_of(procfs: &ProcFs, pid: u32) -> String {
    procfs
        .read_string(&format!("{pid}/comm"))
        .map(|raw| raw.trim().to_string())
        .unwrap_or_default()
}

/// 查询目标的匹配基准：原路径与符号链接归一化路径（witr `ResolveFile` 的
/// `linkPath == realPath || linkPath == absPath` 双比较语义）。
fn canonical_target(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

impl LinuxPlatform {
    /// 进程全部 FD 链接目标（按 fd 目录项 readlink；读不到的项跳过）。
    fn fd_links(&self, pid: u32) -> Vec<PathBuf> {
        let Ok(names) = self.procfs.read_dir_names(&format!("{pid}/fd")) else {
            return Vec::new();
        };
        names
            .iter()
            .filter_map(|name| self.procfs.read_link(&format!("{pid}/fd/{name}")).ok())
            .collect()
    }

    /// 进程的 FD → inode → 链接目标映射（按 inode 单独匹配——witr `statKey`
    /// 注释：/proc/locks 的 device:inode 与用户态 stat 的设备号格式不完全
    /// 一致；跨文件系统 inode 撞号在实践中可忽略）。
    fn fd_inode_map(&self, pid: u32) -> HashMap<u64, PathBuf> {
        let Ok(names) = self.procfs.read_dir_names(&format!("{pid}/fd")) else {
            return HashMap::new();
        };
        let mut map = HashMap::new();
        for name in names {
            let Ok(target) = self.procfs.read_link(&format!("{pid}/fd/{name}")) else {
                continue;
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
    fn lock_entries(
        &self,
        path_filter: Option<(&Path, &Path, Option<u64>)>,
    ) -> (Vec<FileLockEntry>, Vec<DiagnosticIssue>) {
        let raw = match self.procfs.read_string("locks") {
            Ok(raw) => raw,
            Err(error) if error.kind() == ErrorKind::NotFound => return (Vec::new(), Vec::new()),
            Err(error) => {
                return (
                    Vec::new(),
                    vec![DiagnosticIssue::new(
                        if error.kind() == ErrorKind::PermissionDenied {
                            DiagnosticCode::PermissionDenied
                        } else {
                            DiagnosticCode::Unknown
                        },
                        format!("/proc/locks 不可读：{error}"),
                    )],
                );
            }
        };
        let mut fd_cache: HashMap<u32, HashMap<u64, PathBuf>> = HashMap::new();
        let mut issues = Vec::new();
        let mut entries = Vec::new();
        for row in parse_locks(&raw) {
            let Some(mode) = lock_mode_of(&row.mode) else {
                issues.push(DiagnosticIssue::new(
                    DiagnosticCode::ParseFailed,
                    format!("/proc/locks 行模式 {} 不可识别，已跳过", row.mode),
                ));
                continue;
            };
            let holder_fds = fd_cache
                .entry(row.pid)
                .or_insert_with(|| self.fd_inode_map(row.pid));
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
            entries.push(FileLockEntry {
                pid: Pid::new(row.pid).unwrap_or(Pid::MIN),
                process: process_name_of(&self.procfs, row.pid),
                path: resolved,
                lock_type: lock_type_of(&row.lock_type),
                mode,
            });
        }
        (entries, issues)
    }
}

impl FileInventory for LinuxPlatform {
    fn capability(&self) -> CapabilityStatus {
        CapabilityStatus::Supported
    }

    fn holders(&self, path: &Path) -> Inspection<Vec<FileLockEntry>> {
        let target = path.to_path_buf();
        let canonical = canonical_target(path);
        let target_ino = fs::metadata(&canonical).ok().map(|meta| meta.ino());

        // 锁记录优先：先收锁条目（含解析诊断），再以普通 FD 补齐未覆盖的
        // （pid, path）。
        let (mut entries, issues) = self.lock_entries(Some((&target, &canonical, target_ino)));
        let locked: HashSet<(u32, PathBuf)> = entries
            .iter()
            .map(|entry| (entry.pid.get(), entry.path.clone()))
            .collect();
        let Ok(pids) = self.procfs.list_pids() else {
            return Inspection::failed(vec![DiagnosticIssue::new(
                DiagnosticCode::Unknown,
                String::from("/proc 不可枚举，文件持有者不可得"),
            )]);
        };
        for pid in pids {
            let Ok(holder) = Pid::new(pid) else { continue };
            for link in self.fd_links(pid) {
                if link != canonical && link != target {
                    continue;
                }
                if locked.contains(&(pid, link.clone())) {
                    continue;
                }
                entries.push(FileLockEntry {
                    pid: holder,
                    process: process_name_of(&self.procfs, pid),
                    path: link,
                    // 仅打开（无锁记录）的持有：锁类型/模式无锁语义，用
                    // Other + Read 表达「打开即持有」并避免编造锁模式。
                    lock_type: LockType::Other,
                    mode: LockMode::Read,
                });
            }
        }
        if issues.is_empty() {
            Inspection::complete(entries)
        } else {
            Inspection::partial(entries, issues)
        }
    }
}

impl ProcessFileLocks for LinuxPlatform {
    /// 进程持有的全部文件锁（/proc/locks 按持有者 PID 过滤；parity §1
    /// `FileContext.LockedFiles`）。与 [`FileInventory::holders`] 共享同一
    /// 解析路径；锁路径解析失败时回退 `dev:ino` 原文并携带诊断。
    fn locks_of(&self, pid: Pid) -> Inspection<Vec<FileLockEntry>> {
        let (entries, issues) = self.lock_entries(None);
        let owned: Vec<FileLockEntry> = entries
            .into_iter()
            .filter(|entry| entry.pid == pid)
            .collect();
        if issues.is_empty() {
            Inspection::complete(owned)
        } else {
            Inspection::partial(owned, issues)
        }
    }
}
