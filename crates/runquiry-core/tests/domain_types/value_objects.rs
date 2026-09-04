//! PID、端口、renice 与进程身份的值对象边界。

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use runquiry_core::{Pid, Port, ProcessIdentity, Renice};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn pid_rejects_zero() {
    assert!(Pid::new(0).is_err());
    assert_eq!(Pid::new(1).map(Pid::get), Ok(1));
    assert_eq!(Pid::new(u32::MAX).map(Pid::get), Ok(u32::MAX));
}

#[test]
fn port_must_be_between_one_and_max() {
    assert!(Port::new(0).is_err());
    assert_eq!(Port::new(1).map(Port::get), Ok(1));
    assert_eq!(Port::new(65535).map(Port::get), Ok(65535));
}

#[test]
fn renice_accepts_boundary_values() {
    assert!(Renice::try_from(-20).is_ok());
    assert!(Renice::try_from(19).is_ok());
    assert!(Renice::try_from(-21).is_err());
    assert!(Renice::try_from(20).is_err());
    assert_eq!(Renice::try_from(-7).map(Renice::get), Ok(-7));
}

#[test]
fn renice_cannot_be_built_from_serde_out_of_range() {
    let round: Result<Renice, _> = serde_json::from_str("25");
    assert!(round.is_err(), "越界 Renice 不得通过反序列化构造");
}

#[test]
fn process_identity_change_semantics() -> TestResult {
    let started = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000);
    let executable = PathBuf::from("/usr/bin/nginx");
    let other_executable = PathBuf::from("/usr/sbin/nginx");
    let pid = Pid::new(4242)?;
    let first = ProcessIdentity::new(pid, Some(started), Some(executable.clone()));
    let same = ProcessIdentity::new(pid, Some(started), Some(executable.clone()));
    let reused = ProcessIdentity::new(
        pid,
        Some(started + Duration::from_secs(5)),
        Some(executable.clone()),
    );
    let renamed = ProcessIdentity::new(pid, Some(started), Some(other_executable));
    let unknown_first = ProcessIdentity::new(pid, None, Some(executable.clone()));
    let unknown_second = ProcessIdentity::new(pid, None, Some(executable));
    assert!(first.same_process(&same));
    assert_eq!(first.pid(), reused.pid());
    assert!(!first.same_process(&reused));
    assert_ne!(first.start_time(), reused.start_time());
    assert!(first.same_process(&renamed));
    assert!(!first.same_process(&unknown_first) && !unknown_first.same_process(&first));
    assert!(!unknown_first.same_process(&unknown_second));
    Ok(())
}
