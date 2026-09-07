//! Linux production backend 的真实目标解析回归测试。

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, BufRead as _, Read as _, Write as _};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, mpsc};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use runquiry_core::{
    CapabilityStatus, FileInventory, InspectError, NetworkInventory, Pid, Port, ProcessAction,
    ProcessIdentity, ProcessInventory, QueryTarget, Resolution,
};
use runquiry_platform::linux::LinuxPlatform;
use runquiry_ui::backend::{InvestigationTarget, WorkspaceBackend};

use super::{PlatformBackend, UnavailableBackend};

const PORT_READY: &str = "RUNQUIRY_BACKEND_PORT=";
const FILE_READY: &str = "RUNQUIRY_BACKEND_FILE_READY";

/// 父测试启动的、仅持有一个受控资源的独立测试进程。
struct TargetHelper {
    child: Child,
    stdout: io::BufReader<ChildStdout>,
    path: PathBuf,
}

impl TargetHelper {
    fn port() -> io::Result<(Self, Port)> {
        let mut helper = Self::spawn(
            "backend::tests::port_target_helper",
            "RUNQUIRY_BACKEND_PORT",
            OsString::from("1"),
        )?;
        let port = helper.read_port()?;
        Ok((helper, port))
    }

    fn file() -> io::Result<(Self, PathBuf)> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?;
        let path = std::env::temp_dir().join(format!(
            "runquiry-backend-target-{}-{}",
            std::process::id(),
            elapsed.as_nanos()
        ));
        if path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "临时目标路径已存在",
            ));
        }
        let mut helper = Self::spawn(
            "backend::tests::file_target_helper",
            "RUNQUIRY_BACKEND_FILE",
            path.clone().into_os_string(),
        )?;
        helper.wait_for(FILE_READY)?;
        helper.path = path.clone();
        Ok((helper, path))
    }

    fn spawn(test: &str, key: &str, value: OsString) -> io::Result<Self> {
        let binary = std::env::current_exe()?;
        let mut child = Command::new(binary)
            .arg("--exact")
            .arg(test)
            .arg("--nocapture")
            .env(key, value)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("受控子进程缺少 stdout"))?;
        Ok(Self {
            child,
            stdout: io::BufReader::new(stdout),
            path: PathBuf::new(),
        })
    }

    fn wait_for(&mut self, expected: &str) -> io::Result<()> {
        for _ in 0..32 {
            let mut line = String::new();
            if self.stdout.read_line(&mut line)? == 0 {
                return Err(io::Error::other("受控子进程在就绪前退出"));
            }
            if line.trim() == expected {
                return Ok(());
            }
        }
        Err(io::Error::other("受控子进程没有输出就绪标记"))
    }

    fn read_port(&mut self) -> io::Result<Port> {
        for _ in 0..32 {
            let mut line = String::new();
            if self.stdout.read_line(&mut line)? == 0 {
                return Err(io::Error::other("端口子进程在就绪前退出"));
            }
            if let Some(raw) = line.trim().strip_prefix(PORT_READY) {
                let value = raw.parse::<u16>().map_err(|error| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("端口就绪值无效：{error}"),
                    )
                })?;
                return Port::new(value)
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error));
            }
        }
        Err(io::Error::other("端口子进程没有输出端口标记"))
    }

    fn pid(&self) -> Result<Pid, runquiry_core::InvalidId> {
        Pid::new(self.child.id())
    }

    fn finish(mut self) -> io::Result<()> {
        self.child.stdin.take();
        let status = self.child.wait()?;
        if !status.success() {
            return Err(io::Error::other("受控子进程未成功退出"));
        }
        if !self.path.as_os_str().is_empty() && self.path.exists() {
            return Err(io::Error::other("受控临时文件未清理"));
        }
        Ok(())
    }
}

impl Drop for TargetHelper {
    fn drop(&mut self) {
        self.child.stdin.take();
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
        if !self.path.as_os_str().is_empty() {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn result_contains_process(resolution: Resolution<InvestigationTarget>, pid: Pid) -> bool {
    resolution.into_candidates().into_iter().any(
        |target| matches!(target, InvestigationTarget::Process(identity) if identity.pid() == pid),
    )
}

fn record_maximum(maximum: &AtomicUsize, value: usize) {
    let mut observed = maximum.load(Ordering::Acquire);
    while value > observed {
        match maximum.compare_exchange(observed, value, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return,
            Err(current) => observed = current,
        }
    }
}

#[test]
fn serializes_concurrent_analysis_admission() -> Result<(), Box<dyn std::error::Error>> {
    let backend = Arc::new(PlatformBackend::new()?);
    let barrier = Arc::new(Barrier::new(3));
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let (entered_tx, entered_rx) = mpsc::channel();
    let mut releases = Vec::new();
    let mut workers = Vec::new();

    for index in 0..2 {
        let (release_tx, release_rx) = mpsc::channel();
        releases.push(release_tx);
        let backend = Arc::clone(&backend);
        let barrier = Arc::clone(&barrier);
        let active = Arc::clone(&active);
        let maximum = Arc::clone(&maximum);
        let entered = entered_tx.clone();
        workers.push(thread::spawn(move || {
            barrier.wait();
            backend.with_analysis_gate(|| {
                let concurrent = active.fetch_add(1, Ordering::AcqRel) + 1;
                record_maximum(&maximum, concurrent);
                entered.send(index).map_err(|_| InspectError::Unsupported {
                    reason: String::from("analysis gate probe receiver closed"),
                })?;
                release_rx.recv().map_err(|_| InspectError::Unsupported {
                    reason: String::from("analysis gate probe release closed"),
                })?;
                active.fetch_sub(1, Ordering::AcqRel);
                Ok(())
            })
        }));
    }
    drop(entered_tx);
    barrier.wait();

    for _ in 0..2 {
        let index = entered_rx.recv()?;
        releases[index].send(())?;
    }
    for worker in workers {
        worker
            .join()
            .map_err(|_| io::Error::other("analysis gate probe thread panicked"))??;
    }
    assert_eq!(maximum.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn forwards_a_process_action_failure_without_reclassification()
-> Result<(), Box<dyn std::error::Error>> {
    let backend = PlatformBackend::new()?;
    let impossible_pid = Pid::new(999_999_999)?;
    let identity = ProcessIdentity::new(impossible_pid, Some(SystemTime::UNIX_EPOCH), None);

    let result = backend.execute_process_action(&identity, ProcessAction::Terminate);

    assert!(matches!(
        result,
        Err(InspectError::NotFound { subject }) if subject == "进程 999999999"
    ));
    Ok(())
}

#[test]
fn unavailable_backend_disables_process_actions_with_its_construction_reason() {
    let backend = UnavailableBackend::new("Linux 平台采集器不可用");
    let identity = ProcessIdentity::new(Pid::MIN, Some(SystemTime::UNIX_EPOCH), None);

    assert_eq!(
        backend.process_control_capability(),
        CapabilityStatus::Unsupported(String::from("Linux 平台采集器不可用")),
    );
    assert!(matches!(
        backend.execute_process_action(&identity, ProcessAction::Terminate),
        Err(InspectError::Unsupported { reason }) if reason == "Linux 平台采集器不可用"
    ));
}

#[test]
fn executes_confirmed_actions_against_a_task_owned_process()
-> Result<(), Box<dyn std::error::Error>> {
    let backend = PlatformBackend::new()?;
    let (mut helper, _) = TargetHelper::port()?;
    let pid = helper.pid()?;
    eprintln!("B7 app action test created task-owned PID {pid}");
    let inventory = ProcessInventory::list(&LinuxPlatform::new()?)
        .data
        .ok_or_else(|| io::Error::other("进程清单采集未返回数据"))?;
    let identity = inventory
        .iter()
        .find(|process| process.identity.pid() == pid)
        .map(|process| process.identity.clone())
        .ok_or_else(|| io::Error::other("任务自建进程未出现在进程清单中"))?;

    backend.execute_process_action(&identity, ProcessAction::Pause)?;
    backend.execute_process_action(&identity, ProcessAction::Resume)?;
    backend.execute_process_action(&identity, ProcessAction::Terminate)?;

    let status = helper.child.wait()?;
    assert!(!status.success());
    eprintln!("B7 app action test terminated and reaped task-owned PID {pid}: {status}");
    Ok(())
}

#[test]
fn resolves_post_start_loopback_port_holder() -> Result<(), Box<dyn std::error::Error>> {
    let stale_platform = LinuxPlatform::new()?;
    let backend = PlatformBackend::new()?;
    let (helper, port) = TargetHelper::port()?;
    let owner = helper.pid()?;
    let owners = NetworkInventory::open_ports(&LinuxPlatform::new()?)
        .data
        .ok_or_else(|| io::Error::other("端口采集未返回数据"))?;
    assert!(
        owners
            .iter()
            .any(|entry| entry.port == port && entry.pid == Some(owner))
    );
    let stale_inventory = ProcessInventory::list(&stale_platform)
        .data
        .ok_or_else(|| io::Error::other("旧进程快照未返回数据"))?;
    assert!(
        !stale_inventory
            .iter()
            .any(|process| process.identity.pid() == owner)
    );
    let resolution = backend.resolve(&QueryTarget::Port(port))?;

    assert!(result_contains_process(resolution, owner));
    helper.finish()?;
    Ok(())
}

#[test]
fn resolves_post_start_open_file_holder() -> Result<(), Box<dyn std::error::Error>> {
    let stale_platform = LinuxPlatform::new()?;
    let backend = PlatformBackend::new()?;
    let (helper, path) = TargetHelper::file()?;
    let owner = helper.pid()?;
    let holders = FileInventory::holders(&LinuxPlatform::new()?, &path)
        .data
        .ok_or_else(|| io::Error::other("文件采集未返回数据"))?;
    assert!(holders.iter().any(|entry| entry.pid == owner));
    let stale_inventory = ProcessInventory::list(&stale_platform)
        .data
        .ok_or_else(|| io::Error::other("旧进程快照未返回数据"))?;
    assert!(
        !stale_inventory
            .iter()
            .any(|process| process.identity.pid() == owner)
    );
    let resolution = backend.resolve(&QueryTarget::File(path))?;

    assert!(result_contains_process(resolution, owner));
    helper.finish()?;
    Ok(())
}

#[test]
fn port_target_helper() -> io::Result<()> {
    if std::env::var_os("RUNQUIRY_BACKEND_PORT").is_none() {
        return Ok(());
    }
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let mut output = io::stdout().lock();
    writeln!(output, "{PORT_READY}{}", listener.local_addr()?.port())?;
    output.flush()?;
    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input)?;
    drop(listener);
    Ok(())
}

#[test]
fn file_target_helper() -> io::Result<()> {
    let Some(path) = std::env::var_os("RUNQUIRY_BACKEND_FILE").map(PathBuf::from) else {
        return Ok(());
    };
    let file = File::create(&path)?;
    let mut output = io::stdout().lock();
    writeln!(output, "{FILE_READY}")?;
    output.flush()?;
    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input)?;
    drop(file);
    fs::remove_file(path)?;
    Ok(())
}
