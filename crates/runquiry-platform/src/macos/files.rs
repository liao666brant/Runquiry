//! macOS 文件清单与文件锁（`FileInventory` / `ProcessFileLocks` 的 lsof
//! best-effort 实现）。
//!
//! 语义对齐 witr `openfiles_darwin.go` / `locks_darwin.go`（`lsof -l -n -P`
//! 列格式；REG/DIR 为可见打开文件，FD 列末字符锁标志为真实锁——macOS 无
//! `/proc/locks`，锁类型统一按 witr 记 FLOCK，best-effort 边界已披露）。
//! 同一 (pid, path) 同时出现在锁行与普通 FD 行时锁行优先，不重复输出。
//! `holders(path)` 对齐 witr `target/file_darwin.go::ResolveFile`：
//! `lsof -F p <absPath>` 退出码 1 = 无持有者返回空列表，其他失败报
//! `lsof failed`；随后以路径定向列格式查询补齐明细行。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use runquiry_core::{
    CapabilityStatus, DETAIL_TIMEOUT, DiagnosticCode, DiagnosticIssue, FileInventory,
    FileInventoryEntry, FileLockEntry, Inspection, LockMetadata, LockType, Pid, ProcessFileLocks,
};

use super::MacosPlatform;
use super::lsof;

/// `lsof -l -n -P` 的超时（全系统扫描较慢，取详情级 5s）。
const FILE_SCAN_TIMEOUT: Duration = DETAIL_TIMEOUT;

impl FileInventory for MacosPlatform {
    fn capability(&self) -> CapabilityStatus {
        CapabilityStatus::Partial(String::from(
            "macOS 无 /proc/locks：真实锁经 lsof 锁标志位 best-effort；访问模式与锁标志的歧义不可消除",
        ))
    }

    fn list(&self) -> Inspection<Vec<FileInventoryEntry>> {
        match self.file_rows() {
            Ok((rows, issues)) => {
                let entries = merge_entries(rows);
                if issues.is_empty() {
                    Inspection::complete(entries)
                } else {
                    Inspection::partial(entries, issues)
                }
            }
            Err(issue) => Inspection::failed(vec![issue]),
        }
    }

    fn holders(&self, path: &Path) -> Inspection<Vec<FileInventoryEntry>> {
        let absolute = absolute_path(path);
        // witr ResolveFile 第一段：`lsof -F p <absPath>` 决定「有/无持有者」
        // 与退出码语义（1 = 无持有者返回空，其他失败报 lsof failed）。
        let holders = match self.run_lsof(&["-F", "p", &absolute], FILE_SCAN_TIMEOUT) {
            Ok(output) => output,
            Err(error) => {
                return Inspection::failed(vec![DiagnosticIssue::new(
                    DiagnosticCode::ExternalToolFailed,
                    format!("lsof failed：{error}"),
                )]);
            }
        };
        if holders.exit_code != Some(0) {
            // 退出码 1 = 无进程持有（parity），其余非零与被信号终止一律报
            // lsof failed（witr `ExitError.ExitCode()==1` 分支之外都失败）。
            if holders.exit_code == Some(1) {
                return Inspection::complete(Vec::new());
            }
            return Inspection::failed(vec![DiagnosticIssue::new(
                DiagnosticCode::ExternalToolFailed,
                format!("lsof failed：lsof -F p 退出码 {:?}", holders.exit_code),
            )]);
        }
        let pids: HashSet<u32> = lsof::parse_holder_pids(&String::from_utf8_lossy(&holders.stdout))
            .into_iter()
            .collect();
        if pids.is_empty() {
            return Inspection::complete(Vec::new());
        }
        // 第二段：路径定向列格式查询，补齐明细（进程名 / FD / 锁标志）。
        match self.file_rows_single(&absolute) {
            Ok((rows, issues)) => {
                let entries: Vec<FileInventoryEntry> = merge_entries(rows)
                    .into_iter()
                    .filter(|entry| pids.contains(&entry.pid.get()))
                    .collect();
                if issues.is_empty() {
                    Inspection::complete(entries)
                } else {
                    Inspection::partial(entries, issues)
                }
            }
            Err(issue) => Inspection::failed(vec![issue]),
        }
    }
}

impl ProcessFileLocks for MacosPlatform {
    fn locks_of(&self, pid: Pid) -> Inspection<Vec<FileLockEntry>> {
        let listed = self.list();
        let locks = listed
            .data
            .unwrap_or_default()
            .into_iter()
            .filter(|entry| entry.pid == pid)
            .filter_map(|entry| {
                entry.lock.map(|metadata| FileLockEntry {
                    pid: entry.pid,
                    process: entry.process,
                    path: entry.path,
                    lock_type: metadata.lock_type,
                    mode: metadata.mode,
                })
            })
            .collect();
        if listed.issues.is_empty() {
            Inspection::complete(locks)
        } else {
            Inspection::partial(locks, listed.issues)
        }
    }
}

impl MacosPlatform {
    /// 全系统 `lsof -l -n -P`（非零退出但有 stdout 按抢救语义保留）。
    fn file_rows(&self) -> Result<(Vec<lsof::RawFileRow>, Vec<DiagnosticIssue>), DiagnosticIssue> {
        let output = self
            .run_lsof(&["-l", "-n", "-P"], FILE_SCAN_TIMEOUT)
            .map_err(|error| {
                DiagnosticIssue::new(
                    DiagnosticCode::ExternalToolFailed,
                    format!("lsof 不可用或超时：{error}"),
                )
            })?;
        Ok(self.parse_file_output(output))
    }

    /// 路径定向 `lsof -l -n -P <path>`（holders 的明细查询）。
    fn file_rows_single(
        &self,
        path: &str,
    ) -> Result<(Vec<lsof::RawFileRow>, Vec<DiagnosticIssue>), DiagnosticIssue> {
        let output = self
            .run_lsof(&["-l", "-n", "-P", path], FILE_SCAN_TIMEOUT)
            .map_err(|error| {
                DiagnosticIssue::new(
                    DiagnosticCode::ExternalToolFailed,
                    format!("lsof failed：{error}"),
                )
            })?;
        Ok(self.parse_file_output(output))
    }

    fn parse_file_output(
        &self,
        output: runquiry_core::CommandOutput,
    ) -> (Vec<lsof::RawFileRow>, Vec<DiagnosticIssue>) {
        let mut issues = Vec::new();
        if let Some(issue) = super::exit_salvage_issue(&output, "lsof -l -n -P") {
            issues.push(issue);
        }
        let (rows, parse_issues) = lsof::parse_file_rows(&String::from_utf8_lossy(&output.stdout));
        issues.extend(
            parse_issues
                .into_iter()
                .map(|reason| DiagnosticIssue::new(DiagnosticCode::ParseFailed, reason)),
        );
        (rows, issues)
    }
}

/// 行 → 清单条目合并：锁行优先（`lock: Some`），普通 REG/DIR 行去重后为
/// 可见打开文件（witr `lsofTypeIsFile` + `isInterestingDarwinPath`）。
fn merge_entries(rows: Vec<lsof::RawFileRow>) -> Vec<FileInventoryEntry> {
    let mut locked: HashSet<(u32, String)> = HashSet::new();
    let mut entries: Vec<FileInventoryEntry> = Vec::new();
    // 锁行：不筛 TYPE（witr `ListLockedFiles` 同语义），锁类型统一 FLOCK。
    for row in &rows {
        let Some(mode) = row.lock_mode else {
            continue;
        };
        let Ok(pid) = Pid::new(row.pid) else {
            continue;
        };
        locked.insert((row.pid, row.path.clone()));
        entries.push(FileInventoryEntry {
            pid,
            process: row.process.clone(),
            path: PathBuf::from(&row.path),
            fd: None,
            lock: Some(LockMetadata {
                lock_type: LockType::Flock,
                mode,
            }),
        });
    }
    // 可见打开文件：REG/DIR + 值得呈现的路径 + 未被锁行覆盖。
    for row in rows {
        if !matches!(row.fd_type.as_str(), "REG" | "DIR")
            || !lsof::is_interesting_path(&row.path)
            || locked.contains(&(row.pid, row.path.clone()))
        {
            continue;
        }
        let Ok(pid) = Pid::new(row.pid) else {
            continue;
        };
        entries.push(FileInventoryEntry {
            pid,
            process: row.process,
            path: PathBuf::from(row.path),
            fd: row.fd,
            lock: None,
        });
    }
    entries.sort_by(|left, right| {
        (left.pid, &left.path, left.fd).cmp(&(right.pid, &right.path, right.fd))
    });
    entries
}

/// 目标绝对路径（witr `filepath.Abs` 失败回退原文；再尝试 canonicalize
/// 归一化符号链接——与 Linux `holders` 一致，失败回退绝对路径）。
fn absolute_path(path: &Path) -> String {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_or_else(|_| path.to_path_buf(), |cwd| cwd.join(path))
    };
    std::fs::canonicalize(&absolute)
        .unwrap_or(absolute)
        .to_string_lossy()
        .into_owned()
}
