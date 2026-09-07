//! C1 macOS 控制身份与健康标签的纯决策测试（在 Linux 执行）。
//!
//! 原因：macOS 的 cfg 代码（OS 采集与 FFI）无法在本机编译，故仅通过
//! `#[path]` 引入 src/macos/ 下与 OS 无关的纯决策模块（身份重读比对、
//! 启动时间映射、健康阈值，只依赖 std + runquiry-core）在 Linux 直接编译
//! 运行。OS 侧（kill/setpriority/proc_pidpath）由 macOS 交叉编译检查与
//! 实机 QA 验证。

#![allow(missing_docs)]

#[path = "../src/macos/identity.rs"]
mod identity;

use identity::{health_of, same_control_target, start_time_from_unix_seconds};
use runquiry_core::{HealthStatus, Pid, ProcessIdentity};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const FIXTURE_EPOCH_SECS: u64 = 1_700_000_000;
const FIXTURE_EXE: &str = "/Users/fixture-user/Library/RunquiryFixtures/bin/fxt-app";

fn identity(seconds: Option<u64>, exe: Option<&str>) -> ProcessIdentity {
    ProcessIdentity::new(
        Pid::new(5150).unwrap_or(Pid::MIN),
        seconds.and_then(start_time_from_unix_seconds),
        exe.map(PathBuf::from),
    )
}

#[test]
fn same_control_target_rejects_unverifiable_and_changed_identities() -> TestResult {
    let expected = identity(Some(FIXTURE_EPOCH_SECS), Some(FIXTURE_EXE));
    // 同身份（同启动时间 + 同 exe）：接受。
    assert!(same_control_target(
        &expected,
        &identity(Some(FIXTURE_EPOCH_SECS), Some(FIXTURE_EXE))
    ));
    // current 启动时间不可得（None）：身份不可验证 → 保守拒绝（零副作用）。
    assert!(!same_control_target(
        &expected,
        &identity(None, Some(FIXTURE_EXE))
    ));
    // PID 复用（启动时间不同）：拒绝。
    assert!(!same_control_target(
        &expected,
        &identity(Some(FIXTURE_EPOCH_SECS + 1), Some(FIXTURE_EXE))
    ));
    // exe 展示位不一致：拒绝（Linux controller 同语义）。
    assert!(!same_control_target(
        &expected,
        &identity(
            Some(FIXTURE_EPOCH_SECS),
            Some("/Users/fixture-user/fxt-other")
        )
    ));
    // exe 不可得（proc_pidpath 失败）：拒绝。
    assert!(!same_control_target(
        &expected,
        &identity(Some(FIXTURE_EPOCH_SECS), None)
    ));
    Ok(())
}

#[test]
fn start_time_seconds_zero_means_unavailable() -> TestResult {
    assert_eq!(start_time_from_unix_seconds(0), None);
    assert_eq!(
        start_time_from_unix_seconds(FIXTURE_EPOCH_SECS),
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(FIXTURE_EPOCH_SECS))
    );
    Ok(())
}

#[test]
fn health_thresholds_match_witr_darwin_semantics() -> TestResult {
    assert_eq!(health_of(true, false, 0.0, 0), HealthStatus::Zombie);
    assert_eq!(health_of(false, true, 0.0, 0), HealthStatus::Stopped);
    // cpu > 90 → high-cpu；RSS > 1GiB → high-mem；阈值内 → healthy。
    assert_eq!(health_of(false, false, 90.5, 0), HealthStatus::HighCpu);
    assert_eq!(
        health_of(false, false, 50.0, 1024 * 1024 * 1024),
        HealthStatus::Healthy
    );
    assert_eq!(
        health_of(false, false, 50.0, 1024 * 1024 * 1024 + 1),
        HealthStatus::HighMem
    );
    Ok(())
}
