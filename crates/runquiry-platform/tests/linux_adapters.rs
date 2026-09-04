//! B2 Linux 适配器集成测试：合成 `/proc` 树（tempdir）驱动真实采集代码路径。
//!
//! 不读真实 `/proc`、不依赖 root、不 sleep；权限类场景在 root 环境下探测
//! 后跳过断言（权限位对 root 无效是内核语义，非实现缺陷）。
#![cfg(target_os = "linux")]

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime};

use runquiry_core::{
    ContainerKey, ContainerProcessVerifier, DiagnosticCode, FileInventory, HealthStatus, LockMode,
    LockType, NetworkInventory, Pid, ProcessDetailsProvider, ProcessFileLocks, ProcessIdentity,
    ProcessInventory, SourceEvidenceProvider,
};
use runquiry_platform::linux::LinuxPlatform;

type TestResult = Result<(), Box<dyn std::error::Error>>;

static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

/// 唯一 tempdir 根；测试结束清理（失败遗留不互相污染：路径含唯一序号）。
struct TempProc {
    root: PathBuf,
}

impl TempProc {
    fn new(tag: &str) -> Result<Self, std::io::Error> {
        let root = std::env::temp_dir().join(format!(
            "runquiry-b2-{tag}-{}-{}",
            std::process::id(),
            DIR_SEQ.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn pid_dir(&self, pid: u32) -> PathBuf {
        self.root.join(pid.to_string())
    }

    fn write(&self, rel: &str, content: &str) -> Result<(), std::io::Error> {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)
    }

    fn symlink(&self, rel: &str, target: &str) -> Result<(), std::io::Error> {
        std::os::unix::fs::symlink(target, self.root.join(rel))
    }
}

impl Drop for TempProc {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// `/proc/PID/stat` 合成内容：after-comm 44 个字段，0=state 1=ppid
/// 11=utime 12=stime 19=starttime 21=rss。
fn stat_content(pid: u32, comm: &str, state: char, parent_pid: u32, start_ticks: u64) -> String {
    stat_content_full(
        pid,
        comm,
        state,
        parent_pid,
        start_ticks,
        StatHot::default(),
    )
}

/// stat 热字段覆盖（utime/stime ticks 与 RSS 页数；默认值同 fixture 基线）。
#[derive(Clone, Copy)]
struct StatHot {
    utime: u64,
    stime: u64,
    rss_pages: u64,
}

impl Default for StatHot {
    fn default() -> Self {
        Self {
            utime: 10,
            stime: 5,
            rss_pages: 256,
        }
    }
}

/// 可定制热字段的 stat 合成（字段号同 [`stat_content`] 注释）。
fn stat_content_full(
    pid: u32,
    comm: &str,
    state: char,
    parent_pid: u32,
    start_ticks: u64,
    hot: StatHot,
) -> String {
    let mut fields: Vec<String> = std::iter::repeat_n(String::from("0"), 44).collect();
    fields[0] = state.to_string();
    fields[1] = parent_pid.to_string();
    fields[11] = hot.utime.to_string();
    fields[12] = hot.stime.to_string();
    fields[19] = start_ticks.to_string();
    fields[21] = hot.rss_pages.to_string();
    format!("{pid} ({comm}) {}\n", fields.join(" "))
}

fn status_content(comm: &str, uid: u32, cap_eff: &str) -> String {
    format!(
        "Name:\t{comm}\nState:\tS (sleeping)\nUid:\t{uid}\t{uid}\t{uid}\t{uid}\nCapEff:\t{cap_eff}\n"
    )
}

/// 造一个完整的最小进程目录（stat + status + comm）。
fn write_basic_process(
    temp: &TempProc,
    pid: u32,
    comm: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    temp.write(
        &format!("{pid}/stat"),
        &stat_content(pid, comm, 'S', 1, 100),
    )?;
    temp.write(&format!("{pid}/status"), &status_content(comm, 54321, "0"))?;
    temp.write(&format!("{pid}/comm"), &format!("{comm}\n"))?;
    Ok(())
}

/// 以固定 btime `与平台构造（own_pid` 合成为 9999）。
fn platform_for(temp: &TempProc) -> Result<LinuxPlatform, Box<dyn std::error::Error>> {
    Ok(LinuxPlatform::with_injected(
        temp.root.clone(),
        temp.root.join("run/systemd/system"),
        Pid::new(9_999)?,
    ))
}

const BOOT_TIME_SECS: u64 = 1_700_000_000;
/// btime 1700000000 + `start_ticks` 100 / `CLK_TCK` 100 = 1700000001 秒。
fn expected_start_time() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(BOOT_TIME_SECS + 1)
}

#[path = "linux_adapters/container_membership.rs"]
mod container_membership;
#[path = "linux_adapters/file_locks.rs"]
mod file_locks;
#[path = "linux_adapters/health.rs"]
mod health;
#[path = "linux_adapters/network.rs"]
mod network;
#[path = "linux_adapters/process_details.rs"]
mod process_details;
#[path = "linux_adapters/process_inventory.rs"]
mod process_inventory;
#[path = "linux_adapters/source_evidence.rs"]
mod source_evidence;
