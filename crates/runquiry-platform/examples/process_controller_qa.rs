//! B7 真实进程控制 QA：只操作本程序自建并由 RAII 持有的 sleep 子进程。
#![allow(clippy::print_stdout)] // QA 交付物需记录 PID、状态与清理回执。

// 双 main 模式保证非 Linux 平台 `cargo test --locked` 可编译（与 macos_qa /
// windows_qa 的门控模式一致）；pidfd/renice 边界为 Linux 行为。
#[cfg(not(target_os = "linux"))]
fn main() {
    println!("process_controller_qa 仅可在 Linux 上运行；Windows 侧验证见 tests/windows_*.rs。");
}

#[cfg(target_os = "linux")]
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::process::{Child, Command, Stdio};
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use runquiry_core::{
    InspectError, ProcessAction, ProcessController, ProcessIdentity, ProcessInventory, Renice,
};
#[cfg(target_os = "linux")]
use runquiry_platform::linux::LinuxPlatform;

#[cfg(target_os = "linux")]
type QaResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[cfg(target_os = "linux")]
struct OwnedChild(Option<Child>);

#[cfg(target_os = "linux")]
impl OwnedChild {
    fn spawn() -> Result<Self, std::io::Error> {
        Command::new("sleep")
            .arg("60")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|child| Self(Some(child)))
    }

    fn pid(&self) -> QaResult<u32> {
        self.0
            .as_ref()
            .map(Child::id)
            .ok_or_else(|| String::from("子进程已回收").into())
    }

    fn alive(&mut self) -> QaResult<bool> {
        Ok(self
            .0
            .as_mut()
            .ok_or_else(|| String::from("子进程已回收"))?
            .try_wait()?
            .is_none())
    }

    fn wait_and_reap(&mut self) -> QaResult {
        let pid = self.pid()?;
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if self
                .0
                .as_mut()
                .ok_or_else(|| String::from("子进程已回收"))?
                .try_wait()?
                .is_some()
            {
                self.0 = None;
                println!("pid={pid} cleanup=reaped");
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        Err(format!("PID {pid} 未在期限内退出").into())
    }
}

#[cfg(target_os = "linux")]
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(target_os = "linux")]
fn identity(platform: &LinuxPlatform, pid: u32) -> QaResult<ProcessIdentity> {
    ProcessInventory::list(platform)
        .data
        .ok_or_else(|| String::from("无法读取真实进程基线"))?
        .into_iter()
        .find(|summary| summary.identity.pid().get() == pid)
        .map(|summary| summary.identity)
        .ok_or_else(|| format!("基线缺少 QA 子进程 {pid}").into())
}

#[cfg(target_os = "linux")]
fn state_and_nice(pid: u32) -> QaResult<(char, i8)> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))?;
    let close = stat
        .rfind(')')
        .ok_or_else(|| String::from("stat 缺少 comm 结束符"))?;
    let fields: Vec<&str> = stat[close + 1..].split_whitespace().collect();
    let state = fields
        .first()
        .and_then(|field| field.chars().next())
        .ok_or_else(|| String::from("stat 缺少 state"))?;
    let nice = fields
        .get(16)
        .ok_or_else(|| String::from("stat 缺少 nice"))?
        .parse::<i8>()?;
    Ok((state, nice))
}

#[cfg(target_os = "linux")]
fn wait_stopped(pid: u32, expected: bool) -> QaResult<char> {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        let state = state_and_nice(pid)?.0;
        if (state == 'T') == expected {
            return Ok(state);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    Err(format!("PID {pid} 未切换 stopped={expected}").into())
}

#[cfg(target_os = "linux")]
fn main() -> QaResult {
    let mut graceful = OwnedChild::spawn()?;
    let graceful_pid = graceful.pid()?;
    let platform = LinuxPlatform::new()?;
    let expected = identity(&platform, graceful_pid)?;
    println!(
        "pid={graceful_pid} owned=true initial_state={}",
        state_and_nice(graceful_pid)?.0
    );

    let false_identity = ProcessIdentity::new(
        expected.pid(),
        expected.start_time(),
        Some(PathBuf::from("/runquiry/false-executable")),
    );
    let rejected = ProcessController::execute(&platform, &false_identity, ProcessAction::Terminate);
    if !matches!(rejected, Err(InspectError::ProcessChanged { .. })) || !graceful.alive()? {
        return Err(String::from("伪身份未被安全拒绝或子进程未存活").into());
    }
    println!("pid={graceful_pid} fake_identity=process_changed alive=true");

    ProcessController::execute(&platform, &expected, ProcessAction::Pause)?;
    println!(
        "pid={graceful_pid} action=pause state={}",
        wait_stopped(graceful_pid, true)?
    );
    ProcessController::execute(&platform, &expected, ProcessAction::Resume)?;
    println!(
        "pid={graceful_pid} action=resume state={}",
        wait_stopped(graceful_pid, false)?
    );
    ProcessController::execute(
        &platform,
        &expected,
        ProcessAction::Renice(Renice::try_from(19)?),
    )?;
    println!(
        "pid={graceful_pid} action=renice nice={}",
        state_and_nice(graceful_pid)?.1
    );
    ProcessController::execute(&platform, &expected, ProcessAction::Terminate)?;
    println!("pid={graceful_pid} action=terminate sent=true");
    graceful.wait_and_reap()?;

    let mut forced = OwnedChild::spawn()?;
    let forced_pid = forced.pid()?;
    let forced_platform = LinuxPlatform::new()?;
    let forced_identity = identity(&forced_platform, forced_pid)?;
    println!(
        "pid={forced_pid} owned=true initial_state={}",
        state_and_nice(forced_pid)?.0
    );
    ProcessController::execute(&forced_platform, &forced_identity, ProcessAction::Kill)?;
    println!("pid={forced_pid} action=kill sent=true");
    forced.wait_and_reap()?;

    println!("qa=pass owned_processes=2 cleanup=complete");
    Ok(())
}
