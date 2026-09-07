//! B7 Linux 进程控制边界回归。
#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use runquiry_core::{
    InspectError, Pid, ProcessAction, ProcessController, ProcessIdentity, ProcessInventory, Renice,
};
use runquiry_platform::linux::LinuxPlatform;

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn_sleep() -> Result<ChildGuard, std::io::Error> {
    Command::new("sleep")
        .arg("60")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(ChildGuard)
}

fn listed_identity(platform: &LinuxPlatform, pid: u32) -> Result<ProcessIdentity, String> {
    ProcessInventory::list(platform)
        .data
        .ok_or_else(|| String::from("无法读取真实进程基线"))?
        .into_iter()
        .find(|summary| summary.identity.pid().get() == pid)
        .map(|summary| summary.identity)
        .ok_or_else(|| format!("基线缺少测试子进程 {pid}"))
}

fn proc_state_and_nice(pid: u32) -> Result<(char, i8), Box<dyn std::error::Error>> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))?;
    let close = stat
        .rfind(')')
        .ok_or_else(|| String::from("测试子进程 stat 缺少 comm 结束符"))?;
    let fields: Vec<&str> = stat[close + 1..].split_whitespace().collect();
    let state = fields
        .first()
        .and_then(|field| field.chars().next())
        .ok_or_else(|| String::from("测试子进程 stat 缺少 state"))?;
    let nice = fields
        .get(16)
        .ok_or_else(|| String::from("测试子进程 stat 缺少 nice"))?
        .parse::<i8>()?;
    Ok((state, nice))
}

fn wait_for_state(pid: u32, expected_stopped: bool) -> TestResult {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        let stopped = proc_state_and_nice(pid)?.0 == 'T';
        if stopped == expected_stopped {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    Err(format!("PID {pid} 未在期限内切换 stopped={expected_stopped}").into())
}

fn wait_for_exit(child: &mut Child) -> TestResult {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    Err(format!("PID {} 未在期限内退出", child.id()).into())
}

#[test]
fn process_controller_rejects_injected_procfs_without_side_effects() -> TestResult {
    // Given：注入式平台携带一个恰好等于当前真实进程的 PID。
    let pid = Pid::new(std::process::id())?;
    let platform = LinuxPlatform::with_injected(
        PathBuf::from("/proc"),
        PathBuf::from("/run/systemd/system"),
        Pid::MIN,
    );
    let expected = runquiry_core::ProcessIdentity::new(
        pid,
        Some(std::time::SystemTime::UNIX_EPOCH),
        Some(PathBuf::from("/does/not/matter")),
    );

    // When：调用破坏性动作。
    let result = ProcessController::execute(&platform, &expected, ProcessAction::Kill);

    // Then：测试注入边界必须拒绝，测试进程仍能执行到此处。
    assert!(matches!(result, Err(InspectError::Unsupported { .. })));
    Ok(())
}

#[test]
fn process_controller_rejects_executable_mismatch_and_child_survives() -> TestResult {
    // Given：控制器构造前启动且由本测试持有的临时进程。
    let mut child = spawn_sleep()?;
    let child_pid = child.0.id();
    let platform = LinuxPlatform::new()?;
    let current = listed_identity(&platform, child_pid)?;
    let expected = runquiry_core::ProcessIdentity::new(
        current.pid(),
        current.start_time(),
        Some(PathBuf::from("/definitely/not/the-sleep-executable")),
    );

    // When：以启动时间相同但 exe 不同的确认快照执行终止。
    let result = ProcessController::execute(&platform, &expected, ProcessAction::Terminate);

    // Then：额外 exe 核验拒绝动作，临时进程仍存活。
    assert!(matches!(result, Err(InspectError::ProcessChanged { .. })));
    assert!(child.0.try_wait()?.is_none());
    Ok(())
}

#[test]
fn process_controller_rejects_reused_or_unverifiable_identity_and_child_survives() -> TestResult {
    // Given：一个真实自建子进程，以及从生产基线读取的完整身份。
    let mut child = spawn_sleep()?;
    let child_pid = child.0.id();
    let platform = LinuxPlatform::new()?;
    let current = listed_identity(&platform, child_pid)?;
    let executable = current
        .executable()
        .cloned()
        .ok_or_else(|| String::from("测试子进程必须具有可验证 exe"))?;
    let changed_start = current
        .start_time()
        .and_then(|start| start.checked_add(Duration::from_secs(1)))
        .ok_or_else(|| String::from("测试子进程必须具有可验证启动时间"))?;
    let cases = [
        (
            "pid_reused_start_changed",
            ProcessIdentity::new(current.pid(), Some(changed_start), Some(executable.clone())),
        ),
        (
            "start_time_unknown",
            ProcessIdentity::new(current.pid(), None, Some(executable)),
        ),
        (
            "executable_unknown",
            ProcessIdentity::new(current.pid(), current.start_time(), None),
        ),
    ];

    for (case, expected) in cases {
        // When：每种不可证明身份都请求温和终止。
        let result = ProcessController::execute(&platform, &expected, ProcessAction::Terminate);

        // Then：全部按 ProcessChanged 拒绝，且真实子进程持续存活。
        assert!(
            matches!(result, Err(InspectError::ProcessChanged { .. })),
            "case={case}, result={result:?}"
        );
        assert!(child.0.try_wait()?.is_none(), "case={case}");
    }
    Ok(())
}

#[test]
fn process_controller_pause_resume_renice_and_terminate_owned_child() -> TestResult {
    // Given：任务自建并由 RAII 守卫持有的 sleep 子进程。
    let mut child = spawn_sleep()?;
    let pid = child.0.id();
    let platform = LinuxPlatform::new()?;
    let identity = listed_identity(&platform, pid)?;

    // When/Then：暂停与恢复都通过 pidfd 定向，状态由真实 /proc 观测。
    ProcessController::execute(&platform, &identity, ProcessAction::Pause)?;
    wait_for_state(pid, true)?;
    ProcessController::execute(&platform, &identity, ProcessAction::Resume)?;
    wait_for_state(pid, false)?;

    // When/Then：renice 到普通用户必可设置的较低优先级，并读取真实 nice 字段。
    ProcessController::execute(
        &platform,
        &identity,
        ProcessAction::Renice(Renice::try_from(19)?),
    )?;
    assert_eq!(proc_state_and_nice(pid)?.1, 19);

    // When/Then：温和终止后子进程在期限内退出且被 wait 回收。
    ProcessController::execute(&platform, &identity, ProcessAction::Terminate)?;
    wait_for_exit(&mut child.0)?;
    Ok(())
}

#[test]
fn process_controller_kills_owned_child() -> TestResult {
    // Given：独立的第二个自建子进程及其真实身份。
    let mut child = spawn_sleep()?;
    let pid = child.0.id();
    let platform = LinuxPlatform::new()?;
    let identity = listed_identity(&platform, pid)?;

    // When：发出强制终止。
    ProcessController::execute(&platform, &identity, ProcessAction::Kill)?;

    // Then：真实子进程退出并被回收。
    wait_for_exit(&mut child.0)?;
    Ok(())
}

#[test]
fn process_controller_rejects_own_process_before_any_syscall() -> TestResult {
    // Given：expected 指向当前测试进程；字段内容不影响自身保护。
    let platform = LinuxPlatform::new()?;
    let expected = ProcessIdentity::new(
        Pid::new(std::process::id())?,
        Some(std::time::SystemTime::UNIX_EPOCH),
        Some(PathBuf::from("/irrelevant")),
    );

    // When：请求暂停自身。
    let result = ProcessController::execute(&platform, &expected, ProcessAction::Pause);

    // Then：在 pidfd/syscall 之前拒绝，否则测试无法执行到断言。
    assert!(matches!(result, Err(InspectError::InvalidTarget { .. })));
    Ok(())
}
