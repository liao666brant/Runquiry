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
    DiagnosticCode, FileInventory, HealthStatus, LockMode, LockType, NetworkInventory, Pid,
    ProcessDetailsProvider, ProcessFileLocks, ProcessIdentity, ProcessInventory,
    SourceEvidenceProvider,
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

#[test]
fn linux_process_baseline_parses_proc_fields() -> TestResult {
    let temp = TempProc::new("baseline")?;
    temp.write(
        "stat",
        &format!("cpu  0\nclockticks 0\nbtime {BOOT_TIME_SECS}\n"),
    )?;
    write_basic_process(&temp, 1, "systemd")?;
    write_basic_process(&temp, 100, "fxt-daemon")?;
    temp.write("100/cmdline", "fxt-daemon --config /etc/fxt.conf\0")?;
    temp.write(
        "100/status",
        &status_content("fxt-daemon", 54321, "00000000a80425fb"),
    )?;
    temp.symlink("100/cwd", "/opt/runquiry-fixtures/var")?;
    temp.symlink("100/exe", "/usr/bin/fxt-daemon")?;
    temp.write(
        "100/cgroup",
        "0::/system.slice/docker-abc123def456abc123def456abc123def456abc123def456abc123def456abc1.scope\n",
    )?;
    let platform = platform_for(&temp)?;

    let listed = ProcessInventory::list(&platform);
    let summaries = listed
        .data
        .as_deref()
        .ok_or_else(|| String::from("基线采集不应整体失败"))?;
    assert_eq!(summaries.len(), 2, "自身 9999 未入树，应全部保留");
    let entry = summaries
        .iter()
        .find(|entry| entry.identity.pid().get() == 100)
        .ok_or_else(|| String::from("缺少 PID 100"))?;
    assert_eq!(entry.parent_pid, Some(Pid::new(1)?));
    assert_eq!(entry.command, "fxt-daemon");
    assert_eq!(
        entry.command_line.as_deref(),
        Some("fxt-daemon --config /etc/fxt.conf")
    );
    assert_eq!(entry.user.as_deref(), Some("54321"), "未知 uid 回退数字串");
    assert_eq!(entry.health, HealthStatus::Healthy);
    assert!(!entry.exe_deleted);
    let identity = &entry.identity;
    assert_eq!(identity.start_time(), Some(expected_start_time()));
    assert_eq!(
        identity.executable().map(std::path::PathBuf::as_path),
        Some(Path::new("/usr/bin/fxt-daemon"))
    );
    // capabilities 位译码（witr 名单）。
    assert!(entry.capabilities.contains(&String::from("CAP_CHOWN")));
    assert!(
        entry
            .capabilities
            .contains(&String::from("CAP_NET_BIND_SERVICE"))
    );
    assert!(entry.capabilities.contains(&String::from("CAP_SYS_CHROOT")));
    assert!(!entry.capabilities.contains(&String::from("CAP_BPF")));
    // cgroup → core ContainerContext 纯函数。
    let container = entry
        .container
        .as_ref()
        .ok_or_else(|| String::from("docker cgroup 必须给出容器上下文"))?;
    assert_eq!(container.runtime(), "docker");
    assert_eq!(
        container.container_id(),
        "abc123def456abc123def456abc123def456abc123def456abc123def456abc1"
    );
    Ok(())
}

#[test]
fn linux_process_health_distinguishes_zombie_and_stopped() -> TestResult {
    let temp = TempProc::new("health")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 200, "fxt-zombie")?;
    temp.write("200/stat", &stat_content(200, "fxt-zombie", 'Z', 1, 100))?;
    write_basic_process(&temp, 201, "fxt-stopped")?;
    temp.write("201/stat", &stat_content(201, "fxt-stopped", 'T', 1, 100))?;
    let platform = platform_for(&temp)?;

    let summaries = ProcessInventory::list(&platform)
        .data
        .ok_or_else(|| String::from("基线采集不应整体失败"))?;
    let health_of = |pid: u32| {
        summaries
            .iter()
            .find(|entry| entry.identity.pid().get() == pid)
            .map(|entry| entry.health)
    };
    assert_eq!(health_of(200), Some(HealthStatus::Zombie));
    assert_eq!(health_of(201), Some(HealthStatus::Stopped));
    Ok(())
}

#[test]
fn linux_exe_deleted_requires_deleted_suffix_not_permission() -> TestResult {
    let temp = TempProc::new("exe-deleted")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 300, "fxt-deleted")?;
    temp.symlink("300/exe", "/usr/lib/fxt-old (deleted)")?;
    // 无 exe 链接（权限不足/退出）：不得误判为已删除。
    write_basic_process(&temp, 301, "fxt-nolink")?;
    let platform = platform_for(&temp)?;

    let summaries = ProcessInventory::list(&platform)
        .data
        .ok_or_else(|| String::from("基线采集不应整体失败"))?;
    let deleted_of = |pid: u32| {
        summaries
            .iter()
            .find(|entry| entry.identity.pid().get() == pid)
            .map(|entry| (entry.exe_deleted, entry.identity.executable().is_some()))
    };
    assert_eq!(deleted_of(300), Some((true, true)));
    assert_eq!(deleted_of(301), Some((false, false)));
    Ok(())
}

#[test]
fn linux_self_and_late_descendants_are_excluded() -> TestResult {
    let temp = TempProc::new("exclusion")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 1, "systemd")?;
    write_basic_process(&temp, 9_999, "runquiry")?;
    // 基准时点已存在的自身子进程：保留。
    write_basic_process(&temp, 100, "fxt-worker")?;
    temp.write(
        "100/stat",
        &stat_content(100, "fxt-worker", 'S', 9_999, 100),
    )?;
    let platform = platform_for(&temp)?;

    // 平台构造之后再出现的自身后代（模拟采集期辅助进程）：排除。
    temp.write("200/stat", &stat_content(200, "sh", 'S', 9_999, 100))?;
    temp.write("300/stat", &stat_content(300, "grep", 'S', 200, 100))?;
    // 无关进程：保留。
    write_basic_process(&temp, 400, "fxt-unrelated")?;

    let pids: Vec<u32> = ProcessInventory::list(&platform)
        .data
        .unwrap_or_default()
        .iter()
        .map(|entry| entry.identity.pid().get())
        .collect();
    assert!(pids.contains(&100), "基准时点已有的自身子进程保留");
    assert!(pids.contains(&400), "无关进程保留");
    assert!(!pids.contains(&9_999), "自身必须排除");
    assert!(!pids.contains(&200), "采集开始后派生的直接子进程排除");
    assert!(!pids.contains(&300), "采集开始后派生的孙进程排除");
    Ok(())
}

#[test]
fn linux_vanished_and_permission_entries_degrade_to_diagnostics() -> TestResult {
    let temp = TempProc::new("degrade")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 100, "fxt-ok")?;
    // 进程消失：目录在但 stat 不可读（ENOENT 语义）。
    fs::create_dir_all(temp.pid_dir(101))?;
    // 权限不足：stat 存在但目录不可读。
    write_basic_process(&temp, 102, "fxt-secret")?;
    let restricted = temp.pid_dir(102);
    let platform = platform_for(&temp)?;
    fs::set_permissions(&restricted, fs::Permissions::from_mode(0o000))?;

    // root 环境权限位不生效：先探测，命中不了就在收尾时跳过权限断言。
    let permission_enforced = fs::read(restricted.join("stat")).is_err();

    let listed = ProcessInventory::list(&platform);
    let summaries = listed
        .data
        .as_deref()
        .ok_or_else(|| String::from("单点失败不得抹掉全部数据"))?;
    assert_eq!(
        summaries
            .iter()
            .map(|s| s.identity.pid().get())
            .collect::<Vec<_>>(),
        vec![100],
        "PID 100 必须照常返回"
    );
    assert_eq!(summaries[0].identity.pid(), Pid::new(100)?);
    assert!(listed.has_issues());

    let codes: Vec<_> = listed
        .issues
        .iter()
        .map(runquiry_core::DiagnosticIssue::code)
        .collect();
    // stat 缺失 → Unknown（消失）；目录不可读 → PermissionDenied（非 root 时）。
    assert!(
        codes.contains(&DiagnosticCode::Unknown),
        "实际诊断：{codes:?}"
    );
    if permission_enforced {
        assert!(codes.contains(&DiagnosticCode::PermissionDenied));
    }
    fs::set_permissions(&restricted, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

#[test]
fn linux_corrupted_stat_files_are_skipped_without_panic() -> TestResult {
    let temp = TempProc::new("corrupt")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 100, "fxt-ok")?;
    // 无括号定界 / 字段不足 22：均视为格式损坏。
    temp.write("103/stat", "totally garbage\n")?;
    temp.write("104/stat", "104 (fxt-short) S 1 0\n")?;
    let platform = platform_for(&temp)?;

    let listed = ProcessInventory::list(&platform);
    let summaries = listed
        .data
        .as_deref()
        .ok_or_else(|| String::from("损坏条目不得抹掉其余数据"))?;
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].identity.pid(), Pid::new(100)?);
    assert_eq!(listed.issues.len(), 2);
    assert!(
        listed
            .issues
            .iter()
            .all(|issue| issue.code() == DiagnosticCode::Unknown)
    );
    Ok(())
}

#[test]
fn linux_details_reads_extended_proc_info() -> TestResult {
    let temp = TempProc::new("details")?;
    temp.write(
        "stat",
        &format!("cpu  0\nbtime {BOOT_TIME_SECS}\nMemTotal:        1000000 kB\n"),
    )?;
    write_basic_process(&temp, 100, "fxt-daemon")?;
    temp.write("100/cmdline", "fxt-daemon --serve\0")?;
    temp.symlink("100/cwd", "/opt/runquiry-fixtures/var")?;
    temp.write("100/environ", "PATH=/usr/bin\0FXT_MODE=synthetic\0")?;
    temp.write("100/statm", "425 128 64 32 16 48 8\n")?;
    temp.write(
        "100/io",
        "read_bytes: 100\nwrite_bytes: 200\nsyscr: 10\nsyscw: 20\n",
    )?;
    temp.write(
        "100/limits",
        "Limit                     Soft Limit           Hard Limit           Units\n\
         Max open files            1024                 4096                 files\n",
    )?;
    fs::create_dir_all(temp.pid_dir(100).join("fd"))?;
    temp.symlink("100/fd/0", "/dev/null")?;
    temp.symlink("100/fd/3", "socket:[12345]")?;
    temp.symlink("100/fd/4", "/var/log/fxt.log")?;
    // 子进程。
    write_basic_process(&temp, 110, "fxt-child")?;
    temp.write("110/stat", &stat_content(110, "fxt-child", 'S', 100, 100))?;
    let platform = platform_for(&temp)?;

    let identity = ProcessIdentity::new(Pid::new(100)?, Some(expected_start_time()), None);
    let details = ProcessDetailsProvider::details(&platform, &identity)?;
    assert!(details.identity.same_process(&identity));
    assert_eq!(
        details.working_dir,
        Some(PathBuf::from("/opt/runquiry-fixtures/var"))
    );
    assert_eq!(details.environment.len(), 2);
    assert!(
        details
            .environment
            .contains(&(String::from("FXT_MODE"), String::from("synthetic")))
    );
    assert_eq!(details.memory_rss_bytes, Some(128 * 4096));
    let memory = details
        .memory
        .as_ref()
        .ok_or_else(|| String::from("statm 必须产出 MemoryInfo"))?;
    assert_eq!(memory.vms_bytes, 425 * 4096);
    assert_eq!(memory.dirty_bytes, 8 * 4096);
    let io = details
        .io
        .ok_or_else(|| String::from("/proc io 必须产出"))?;
    assert_eq!(io.read_bytes, 100);
    assert_eq!(io.write_ops, 20);
    assert_eq!(details.fd_count, Some(3));
    assert_eq!(details.open_files.len(), 3);
    assert_eq!(details.fd_limit, Some(1024));
    assert_eq!(details.children, vec![Pid::new(110)?]);
    Ok(())
}

#[test]
fn linux_fd_limit_unlimited_is_recorded_as_zero() -> TestResult {
    let temp = TempProc::new("fdlimit")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 104, "fxt-unlimited")?;
    temp.write(
        "104/limits",
        "Limit                     Soft Limit           Hard Limit           Units\n\
         Max open files            unlimited            4096                 files\n",
    )?;
    let platform = platform_for(&temp)?;

    let identity = ProcessIdentity::new(Pid::new(104)?, Some(expected_start_time()), None);
    let details = ProcessDetailsProvider::details(&platform, &identity)?;
    assert_eq!(details.fd_limit, Some(0), "unlimited 契约记 0");
    Ok(())
}

#[test]
fn linux_details_rejects_reused_identity_and_vanished_pid() -> TestResult {
    let temp = TempProc::new("reuse")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 100, "fxt-daemon")?;
    let platform = platform_for(&temp)?;

    // start_time 不一致（PID 复用）：stat 重写为 start_ticks 7000 → 当前
    // 启动时刻为 BOOT+70 秒，与 expected 快照不同 → ProcessChanged。
    temp.write("100/stat", &stat_content(100, "fxt-daemon", 'S', 1, 7_000))?;
    let stale = ProcessIdentity::new(Pid::new(100)?, Some(expected_start_time()), None);
    let result = ProcessDetailsProvider::details(&platform, &stale);
    assert!(matches!(result, Err(..)), "复用身份必须被拒绝");

    // 进程消失 → 拒绝返回数据。
    let missing = ProcessIdentity::new(Pid::new(424_242)?, None, None);
    let result = ProcessDetailsProvider::details(&platform, &missing);
    assert!(matches!(result, Err(..)));
    Ok(())
}
#[test]
fn linux_network_tables_parse_with_inode_attribution() -> TestResult {
    let temp = TempProc::new("network")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 100, "fxt-server")?;
    fs::create_dir_all(temp.pid_dir(100).join("fd"))?;
    temp.symlink("100/fd/3", "socket:[12345]")?;
    temp.symlink("100/fd/4", "socket:[4001]")?;
    // 端口 0000（非法）行：跳过并记 ParseFailed。
    temp.write(
        "net/tcp",
        "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
         0: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 12345 1 ffff8 100 0 0 10 0\n\
         1: 0A000061:0000 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 4444 1 ffff8 100 0 0 10 0\n",
    )?;
    // IPv6 回环 [::1]:9090（每 4 字节组小端序）。
    temp.write(
        "net/tcp6",
        "  sl  local_address remote_address                         st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
         0: 00000000000000000000000001000000:2382 00000000000000000000000000000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 20001 1 ffff8 100 0 0 10 0\n",
    )?;
    temp.write(
        "net/udp",
        "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode ref pointer drops\n\
         0: 00000000:14E9 00000000:0000 07 00000000:00000000 00:00000000 00000000     0        0 30001 1 ffff8 0 0 0 0 0\n",
    )?;
    temp.write(
        "net/unix",
        "Num       RefCount Protocol Flags    Type St Inode Path\n\
         0000000000000000:00000002 00000002 00000000 00010000 0001 03 4001 /run/fxt.sock\n",
    )?;
    let platform = platform_for(&temp)?;

    // open_ports：FD inode 归因 + 无主条目保留为 None + 非法端口跳过记诊断。
    let ports = NetworkInventory::open_ports(&platform);
    let entries = ports
        .data
        .as_deref()
        .ok_or_else(|| String::from("端口采集不应整体失败"))?;
    let attributed = entries
        .iter()
        .find(|entry| entry.pid.map(Pid::get) == Some(100))
        .ok_or_else(|| String::from("缺少归因条目"))?;
    assert_eq!(attributed.port.get(), 8080);
    assert_eq!(attributed.address, "127.0.0.1");
    assert_eq!(attributed.protocol, runquiry_core::Protocol::Tcp);
    assert_eq!(attributed.state, "LISTEN");
    assert!(
        entries
            .iter()
            .any(|entry| entry.pid.is_none() && entry.port.get() == 9090),
        "无主端口必须保留为 pid None"
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.pid.is_none() && entry.port.get() == 5353)
    );
    assert_eq!(ports.issues.len(), 1, "非法端口行必须记 ParseFailed");
    assert_eq!(ports.issues[0].code(), DiagnosticCode::ParseFailed);
    assert!(!entries.iter().any(|entry| entry.port.get() == 0));

    // sockets_of：TCP 归因 + Unix 条目无端口。
    let sockets = NetworkInventory::sockets_of(&platform, Pid::new(100)?);
    let list = sockets
        .data
        .as_deref()
        .ok_or_else(|| String::from("socket 采集不应整体失败"))?;
    let tcp = list
        .iter()
        .find(|entry| entry.protocol == runquiry_core::Protocol::Tcp)
        .ok_or_else(|| String::from("缺少 TCP 条目"))?;
    assert_eq!(tcp.port.map(runquiry_core::Port::get), Some(8080));
    assert_eq!(tcp.inode, Some(12_345));
    assert_eq!(tcp.owner_pid, Some(Pid::new(100)?));
    let unix = list
        .iter()
        .find(|entry| entry.protocol == runquiry_core::Protocol::Unix)
        .ok_or_else(|| String::from("缺少 Unix 条目"))?;
    assert_eq!(unix.port, None, "Unix socket 不得编造假端口");
    assert_eq!(unix.address, "/run/fxt.sock");
    assert_eq!(unix.state, "CONNECTED");
    Ok(())
}

#[test]
fn linux_network_fd_permission_yields_owner_none_with_diagnostic() -> TestResult {
    let temp = TempProc::new("fdperm")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 100, "fxt-server")?;
    write_basic_process(&temp, 105, "fxt-restricted")?;
    fs::create_dir_all(temp.pid_dir(105).join("fd"))?;
    temp.write(
        "net/tcp",
        "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
         0: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 12345 1 ffff8 100 0 0 10 0\n",
    )?;
    let platform = platform_for(&temp)?;

    // 直接以受限 fd 目录探测（root 环境跳过）。
    let restricted = temp.pid_dir(105).join("fd");
    fs::set_permissions(&restricted, fs::Permissions::from_mode(0o000))?;
    let permission_enforced = fs::read_dir(&restricted).is_err();

    let ports = NetworkInventory::open_ports(&platform);
    let entries = ports
        .data
        .as_deref()
        .ok_or_else(|| String::from("端口采集不应整体失败"))?;
    // 条目保留（不静默丢条目），无归因时 pid None。
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].pid, None);
    if permission_enforced {
        assert_eq!(ports.issues[0].code(), DiagnosticCode::PermissionDenied);
    }
    // 还原权限便于 tempdir 清理。
    fs::set_permissions(&restricted, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

#[test]
fn linux_network_unreadable_tables_do_not_fake_success() -> TestResult {
    let temp = TempProc::new("netperm")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    // 无任何 net 表：不得以空集合冒充成功。
    let platform = platform_for(&temp)?;
    let ports = NetworkInventory::open_ports(&platform);
    assert!(ports.is_empty());
    assert_eq!(ports.issues[0].code(), DiagnosticCode::Unknown);

    // 权限失败（非 ENOENT）：整体失败并带 PermissionDenied。
    let temp2 = TempProc::new("netperm2")?;
    temp2.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    temp2.write("net/tcp", "  sl  local_address rem_address   st tx_queue\n")?;
    let restricted = temp2.root.join("net/tcp");
    fs::set_permissions(&restricted, fs::Permissions::from_mode(0o000))?;
    let permission_enforced = fs::read(&restricted).is_err();
    if permission_enforced {
        let platform = platform_for(&temp2)?;
        let ports = NetworkInventory::open_ports(&platform);
        assert!(ports.is_empty());
        assert_eq!(ports.issues[0].code(), DiagnosticCode::PermissionDenied);
        fs::set_permissions(&restricted, fs::Permissions::from_mode(0o644))?;
    }
    Ok(())
}

#[test]
fn linux_locks_parse_with_lock_priority_dedup() -> TestResult {
    let temp = TempProc::new("locks")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 100, "fxt-holder")?;
    write_basic_process(&temp, 101, "fxt-rw")?;
    // 真实文件（fd 链接目标可 stat，inode 匹配生效）。
    let target = temp.root.join("fxt-data.bin");
    fs::write(&target, b"fxt")?;
    let target_ino = fs::metadata(&target)?.ino();
    let second = temp.root.join("fxt-second.bin");
    fs::write(&second, b"fxt2")?;
    let second_ino = fs::metadata(&second)?.ino();
    fs::create_dir_all(temp.pid_dir(100).join("fd"))?;
    temp.symlink("100/fd/5", &format!("{}", target.display()))?;
    temp.symlink("100/fd/6", &format!("{}", target.display()))?;
    fs::create_dir_all(temp.pid_dir(101).join("fd"))?;
    temp.symlink("101/fd/5", &format!("{}", second.display()))?;
    temp.write(
        "locks",
        &format!(
            "1: FLOCK ADVISORY WRITE 100 08:01:{target_ino} 0 EOF\n\
             2: OFDLCK ADVISORY RW 101 08:01:{second_ino} 0 EOF\n\
             3: POSIX ADVISORY WEIRD 102 08:01:{target_ino} 0 0\n"
        ),
    )?;
    let platform = platform_for(&temp)?;

    let holders = FileInventory::holders(&platform, &target);
    let entries = holders
        .data
        .as_deref()
        .ok_or_else(|| String::from("锁采集不应整体失败"))?;
    // 锁记录优先：PID 100 对同一 path 的两个 fd 只产出一条锁记录。
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].pid, Pid::new(100)?);
    assert_eq!(entries[0].process, "fxt-holder");
    assert_eq!(entries[0].path, target);
    assert_eq!(entries[0].lock_type, LockType::Flock);
    assert_eq!(entries[0].mode, LockMode::Write);
    // 未知模式行被跳过并记诊断。
    assert_eq!(holders.issues.len(), 1);
    assert_eq!(holders.issues[0].code(), DiagnosticCode::ParseFailed);

    // OFDLCK + RW。
    let holders2 = FileInventory::holders(&platform, &second);
    let entries2 = holders2
        .data
        .as_deref()
        .ok_or_else(|| String::from("锁采集不应整体失败"))?;
    assert_eq!(entries2.len(), 1);
    assert_eq!(entries2[0].lock_type, LockType::Ofdlck);
    assert_eq!(entries2[0].mode, LockMode::ReadWrite);
    Ok(())
}

#[test]
fn linux_plain_fd_holder_reported_without_duplicating_lock_entry() -> TestResult {
    let temp = TempProc::new("fddup")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 100, "fxt-holder")?;
    let target = temp.root.join("fxt-data.bin");
    fs::write(&target, b"fxt")?;
    let ino = fs::metadata(&target)?.ino();
    fs::create_dir_all(temp.pid_dir(100).join("fd"))?;
    temp.symlink("100/fd/5", &format!("{}", target.display()))?;
    // 只有普通 FD，没有锁记录：以 Other + Read 表达「打开即持有」。
    temp.write("locks", "")?;
    let platform = platform_for(&temp)?;

    let holders = FileInventory::holders(&platform, &target);
    let entries = holders
        .data
        .as_deref()
        .ok_or_else(|| String::from("锁采集不应整体失败"))?;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].pid, Pid::new(100)?);
    assert_eq!(entries[0].lock_type, LockType::Other);
    assert_eq!(entries[0].mode, LockMode::Read);
    assert_eq!(holders.issues.len(), 0);

    // 锁记录 + 同 PID/path 普通 FD → 锁记录优先，不重复。
    temp.write(
        "locks",
        &format!("1: POSIX ADVISORY READ 100 08:01:{ino} 0 EOF\n"),
    )?;
    let holders = FileInventory::holders(&platform, &target);
    let entries = holders
        .data
        .as_deref()
        .ok_or_else(|| String::from("锁采集不应整体失败"))?;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].lock_type, LockType::Posix);
    assert_eq!(entries[0].mode, LockMode::Read);
    Ok(())
}

#[test]
fn linux_source_evidence_collects_raw_inputs_without_judging() -> TestResult {
    let temp = TempProc::new("evidence")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 1, "systemd")?;
    write_basic_process(&temp, 100, "fxt-app")?;
    temp.write("100/cgroup", "0::/system.slice/fxt-app.service\n")?;
    temp.write("1/cgroup", "0::/init.scope\n")?;
    temp.write("100/environ", "SNAP_NAME=fxt-snap\0FXT_SECRET=redacted\0")?;
    // systemd 运行探测目录不存在 → false。
    let platform = platform_for(&temp)?;

    let ancestry = ProcessInventory::list(&platform)
        .data
        .ok_or_else(|| String::from("基线采集不应整体失败"))?;
    // list 升序给出 [1, 100]（根 → 目标）。
    let evidence = SourceEvidenceProvider::evidence(&platform, &ancestry);
    assert!(!evidence.systemd_running);
    assert!(evidence.systemd_details.is_empty());
    assert_eq!(evidence.cgroup_by_pid.len(), 2);
    assert_eq!(evidence.cgroup_by_pid[0].0.get(), 1);
    assert!(evidence.cgroup_by_pid[0].1.contains("init.scope"));
    let env_of_100 = &evidence
        .env_by_pid
        .iter()
        .find(|(owner, _)| owner.get() == 100)
        .ok_or_else(|| String::from("缺少目标进程环境"))?
        .1;
    assert!(env_of_100.contains(&(String::from("SNAP_NAME"), String::from("fxt-snap"))));
    // 敏感性：值不进入任何诊断——本实现不产生诊断，仅确认键值对完整。
    Ok(())
}

#[test]
fn linux_source_evidence_systemd_probe_uses_injected_dir() -> TestResult {
    let temp = TempProc::new("systemd")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 100, "fxt-app")?;
    // systemd 运行目录存在 → systemd_running = true；单元不存在于总线时
    // 富化按 best-effort 省略（无总线/无单元 → details 为空）。
    fs::create_dir_all(temp.root.join("run/systemd/system"))?;
    let platform = platform_for(&temp)?;

    let ancestry = ProcessInventory::list(&platform)
        .data
        .ok_or_else(|| String::from("基线采集不应整体失败"))?;
    let evidence = SourceEvidenceProvider::evidence(&platform, &ancestry);
    assert!(evidence.systemd_running);
    // 合成 cgroup 含 fxt-app.service；真实系统总线没有该单元 → details 空。
    assert!(
        evidence.systemd_details.is_empty(),
        "best-effort 富化失败必须省略而不是伪造"
    );
    Ok(())
}

#[test]
fn linux_capability_status_is_supported_on_linux() -> TestResult {
    let temp = TempProc::new("capability")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    let platform = platform_for(&temp)?;
    assert_eq!(
        ProcessInventory::capability(&platform),
        runquiry_core::CapabilityStatus::Supported
    );
    assert_eq!(
        NetworkInventory::capability(&platform),
        runquiry_core::CapabilityStatus::Supported
    );
    assert_eq!(
        FileInventory::capability(&platform),
        runquiry_core::CapabilityStatus::Supported
    );
    Ok(())
}

/// 健康标签：累计 CPU > 2h → HighCpu（healthy 时判定、优先于 high-mem），
/// RSS > 1GiB → HighMem；阈值边界（恰好 7200s / 1GiB）保持 Healthy
/// （parity witr process_linux.go:193-200）。
#[test]
fn linux_health_labels_compute_high_cpu_and_high_memory() -> TestResult {
    let temp = TempProc::new("health-labels")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    // 700000+700000 ticks = 14000s > 7200s。
    temp.write(
        "100/stat",
        &stat_content_full(
            100,
            "fxt-hot",
            'S',
            1,
            100,
            StatHot {
                utime: 700_000,
                stime: 700_000,
                ..StatHot::default()
            },
        ),
    )?;
    // RSS 262145 页 × 4K = 1073758208 > 1GiB。
    temp.write(
        "101/stat",
        &stat_content_full(
            101,
            "fxt-fat",
            'S',
            1,
            100,
            StatHot {
                rss_pages: 262_145,
                ..StatHot::default()
            },
        ),
    )?;
    // 边界：7200s 整、1GiB 整 → 仍 Healthy。
    temp.write(
        "102/stat",
        &stat_content_full(
            102,
            "fxt-edge",
            'S',
            1,
            100,
            StatHot {
                utime: 719_995,
                rss_pages: 262_144,
                ..StatHot::default()
            },
        ),
    )?;
    for pid in [100_u32, 101, 102] {
        temp.write(&format!("{pid}/status"), &status_content("fxt", 54321, "0"))?;
        temp.write(&format!("{pid}/comm"), "fxt\n")?;
    }
    let platform = platform_for(&temp)?;
    let listed = ProcessInventory::list(&platform);
    let summaries = listed.data.ok_or("应有基线数据")?;
    let health_of = |pid: u32| -> Result<HealthStatus, String> {
        summaries
            .iter()
            .find(|entry| entry.identity.pid().get() == pid)
            .map(|entry| entry.health)
            .ok_or_else(|| String::from("进程应在基线中"))
    };
    assert_eq!(health_of(100)?, HealthStatus::HighCpu);
    assert_eq!(health_of(101)?, HealthStatus::HighMem);
    assert_eq!(health_of(102)?, HealthStatus::Healthy);
    Ok(())
}

/// 自身排除的时间窗兜底：构造时刻之后启动、且父进程已退出（PPID 链断裂，
/// 被 1 号收养）的进程同样被排除（修复前 PPID 链查不到即保留）。
#[test]
fn linux_late_reparented_helper_is_excluded_by_time_window() -> TestResult {
    let temp = TempProc::new("late-reparented")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 1, "systemd")?;
    // 构造时刻：btime + 2s。PID 500 启动于 btime + 5s（晚于构造），父为 1。
    let construction = SystemTime::UNIX_EPOCH + Duration::from_secs(BOOT_TIME_SECS + 2);
    let platform = platform_for(&temp)?.with_constructed_at(construction);
    temp.write("500/stat", &stat_content(500, "fxt-helper", 'S', 1, 500))?;
    temp.write("500/status", &status_content("fxt-helper", 54321, "0"))?;
    temp.write("500/comm", "fxt-helper\n")?;
    let listed = ProcessInventory::list(&platform);
    let summaries = listed.data.ok_or("应有基线数据")?;
    assert!(
        !summaries
            .iter()
            .any(|entry| entry.identity.pid().get() == 500),
        "启动晚于构造时刻的收养后代应被时间窗排除"
    );
    // 构造时刻之前启动的普通进程不受影响。
    assert!(
        summaries
            .iter()
            .any(|entry| entry.identity.pid().get() == 1),
        "既有进程不应被时间窗误排除"
    );
    Ok(())
}

/// 进程级文件锁（`ProcessFileLocks`）：/proc/locks 按持有者 PID 过滤，
/// 与 `holders`（按路径）共享解析路径。
#[test]
fn linux_locks_of_process_filters_by_pid() -> TestResult {
    let temp = TempProc::new("locks-of")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 100, "fxt-holder")?;
    write_basic_process(&temp, 101, "fxt-other")?;
    let target = temp.root.join("fxt-data.bin");
    fs::write(&target, b"fxt")?;
    let target_ino = fs::metadata(&target)?.ino();
    let second = temp.root.join("fxt-second.bin");
    fs::write(&second, b"fxt2")?;
    let second_ino = fs::metadata(&second)?.ino();
    fs::create_dir_all(temp.pid_dir(100).join("fd"))?;
    temp.symlink("100/fd/5", &format!("{}", target.display()))?;
    fs::create_dir_all(temp.pid_dir(101).join("fd"))?;
    temp.symlink("101/fd/5", &format!("{}", second.display()))?;
    temp.write(
        "locks",
        &format!(
            "1: FLOCK ADVISORY WRITE 100 08:01:{target_ino} 0 EOF\n\
             2: OFDLCK ADVISORY RW 101 08:01:{second_ino} 0 EOF\n"
        ),
    )?;
    let platform = platform_for(&temp)?;

    let own = ProcessFileLocks::locks_of(&platform, Pid::new(100)?);
    let entries = own
        .data
        .as_deref()
        .ok_or_else(|| String::from("锁查询不应整体失败"))?;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].pid, Pid::new(100)?);
    assert_eq!(entries[0].lock_type, LockType::Flock);

    let other = ProcessFileLocks::locks_of(&platform, Pid::new(200)?);
    assert!(
        other
            .data
            .as_deref()
            .is_some_and(<[runquiry_core::FileLockEntry]>::is_empty),
        "无锁进程应得到空列表而非失败"
    );
    Ok(())
}
