//! `ProcessController` 契约测试（A5 任务 5）：expected / current 身份比对语义。
//!
//! 验证三组行为：
//! 1. 同 PID 不同 `start_time`（PID 复用）→ [`InspectError::ProcessChanged`]，动作数 0；
//! 2. current `start_time` 为 `None`（身份不可验证）→ 拒绝，动作数 0；
//! 3. 身份一致 → 执行，动作数 1。
//!
//! `executable` 是展示信息，不参与身份判定（parity：`pidIdentityChanged` 只比较
//! PID + `StartedAt`）。

mod support;

use std::time::{Duration, SystemTime};

use runquiry_core::{InspectError, Pid, ProcessAction, ProcessController, ProcessIdentity};
use support::fakes::FakeController;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const PID_VALUE: u32 = 4242;

fn identity(
    start_time: Option<SystemTime>,
    executable: Option<&str>,
) -> Result<ProcessIdentity, Box<dyn std::error::Error>> {
    Ok(ProcessIdentity::new(
        Pid::new(PID_VALUE)?,
        start_time,
        executable.map(std::path::PathBuf::from),
    ))
}

#[test]
fn reused_pid_with_different_start_time_is_rejected_without_side_effect() -> TestResult {
    let expected = identity(
        Some(SystemTime::UNIX_EPOCH),
        Some("/opt/runquiry-fixtures/bin/fxt-daemon"),
    )?;
    let current = identity(
        Some(SystemTime::UNIX_EPOCH + Duration::from_hours(1)),
        Some("/opt/runquiry-fixtures/bin/fxt-daemon"),
    )?;
    let controller = FakeController::new(current);

    let result = controller.execute(&expected, ProcessAction::Terminate);

    let err = result
        .err()
        .ok_or_else(|| String::from("同 PID 不同 start_time 必须返回 ProcessChanged"))?;
    assert_eq!(err.code(), "process_changed");
    if let InspectError::ProcessChanged { identity } = err {
        assert_eq!(
            identity.start_time(),
            Some(SystemTime::UNIX_EPOCH + Duration::from_hours(1)),
            "ProcessChanged 必须携带重读得到的 current 身份"
        );
    }
    assert_eq!(
        controller.executed_count(),
        0,
        "身份不一致时不得产生任何副作用"
    );
    Ok(())
}

#[test]
fn unverifiable_current_start_time_is_rejected_without_side_effect() -> TestResult {
    let expected = identity(Some(SystemTime::UNIX_EPOCH), None)?;
    // 平台重读失败（current start_time 为 None）时身份不可验证，
    // 即使 PID 相同也必须拒绝执行。
    let current = identity(None, None)?;
    let controller = FakeController::new(current);

    let result = controller.execute(&expected, ProcessAction::Kill);

    assert!(
        matches!(result, Err(InspectError::ProcessChanged { .. })),
        "start_time 为 None 时必须拒绝执行"
    );
    assert_eq!(controller.executed_count(), 0, "动作数必须保持 0");
    Ok(())
}

#[test]
fn matching_identity_executes_exactly_once() -> TestResult {
    let started = SystemTime::UNIX_EPOCH;
    let expected = identity(Some(started), Some("/opt/runquiry-fixtures/bin/fxt-daemon"))?;
    let controller = FakeController::new(expected.clone());

    controller.execute(&expected, ProcessAction::Pause)?;

    assert_eq!(controller.executed_count(), 1, "身份一致时恰好执行一次");
    let executed = controller.executed_actions();
    assert_eq!(executed.len(), 1);
    assert_eq!(executed[0].0.get(), PID_VALUE);
    assert_eq!(executed[0].1, ProcessAction::Pause);
    Ok(())
}

#[test]
fn executable_difference_does_not_change_identity() -> TestResult {
    let started = SystemTime::UNIX_EPOCH;
    let expected = identity(Some(started), Some("/opt/runquiry-fixtures/bin/fxt-daemon"))?;
    // current 仅 executable 不同：executable 不参与身份判定，必须照常执行。
    let current = identity(Some(started), Some("/opt/runquiry-fixtures/bin/fxt-worker"))?;
    let controller = FakeController::new(current);

    controller.execute(&expected, ProcessAction::Resume)?;

    assert_eq!(controller.executed_count(), 1);
    Ok(())
}
