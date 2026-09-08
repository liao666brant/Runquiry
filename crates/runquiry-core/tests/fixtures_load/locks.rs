//! PID 复用、文件锁和确定性基建 fixture。

use std::time::Duration;

use runquiry_core::{CapabilityStatus, FileLockEntry, ProcessIdentity};

use crate::support::fixtures::{fixtures_root, load};
use crate::support::{FixedClock, Generation};

use super::metadata::{CAPTURED_AT_MS, PLATFORMS, assert_metadata, expected_captured_at};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn pid_reuse_fixture_provides_distinguishable_identities() -> TestResult {
    for platform in PLATFORMS {
        let fixture = load::<Vec<ProcessIdentity>>(&format!("{platform}/pid-reuse.json"))?;
        assert_metadata(&fixture, platform, "pid_reuse");
        let identities = fixture
            .inspection
            .data
            .as_deref()
            .ok_or_else(|| String::from("pid_reuse 场景应有身份快照"))?;
        assert_eq!(identities.len(), 3);
        let (original, reused, unverifiable) = (&identities[0], &identities[1], &identities[2]);
        assert_eq!(original.pid(), reused.pid());
        assert!(!original.same_process(reused));
        assert!(!original.same_process(unverifiable));
        assert_eq!(unverifiable.start_time(), None);
    }
    Ok(())
}

#[test]
fn windows_file_locks_fixture_marks_capability_unsupported() -> TestResult {
    let fixture = load::<FileLockEntry>("windows/file-locks-unsupported.json")?;
    assert_metadata(&fixture, "windows", "capability_unsupported");
    assert!(fixture.inspection.is_empty());
    assert!(fixture.inspection.issues.is_empty());
    assert_eq!(
        fixture.capability,
        Some(CapabilityStatus::Unsupported(String::from(
            "Windows 无文件锁采集（合成场景）"
        )))
    );
    Ok(())
}

#[test]
fn linux_file_locks_fixture_loads_lock_entries() -> TestResult {
    let fixture = load::<Vec<FileLockEntry>>("linux/file-locks-normal.json")?;
    assert_metadata(&fixture, "linux", "normal");
    let locks = fixture
        .inspection
        .data
        .as_deref()
        .ok_or_else(|| String::from("linux 文件锁快照应有数据"))?;
    assert_eq!(locks.len(), 2);
    assert_eq!(locks[0].lock_type, runquiry_core::LockType::Flock);
    assert_eq!(locks[0].mode, runquiry_core::LockMode::Write);
    assert_eq!(locks[1].lock_type, runquiry_core::LockType::Posix);
    assert_eq!(locks[1].mode, runquiry_core::LockMode::Read);
    assert!(locks.iter().all(|lock| {
        lock.path
            .starts_with(std::path::Path::new("/opt/runquiry-fixtures/"))
    }));
    Ok(())
}

#[test]
fn fixtures_root_and_deterministic_basics_are_stable() -> TestResult {
    assert!(fixtures_root().join("linux").is_dir());
    assert!(fixtures_root().join("windows").is_dir());
    let mut clock = FixedClock::at_epoch_ms(CAPTURED_AT_MS)?;
    assert_eq!(clock.now(), expected_captured_at());
    clock.advance(Duration::from_secs(3));
    assert_eq!(clock.now(), expected_captured_at() + Duration::from_secs(3));
    assert_eq!(Generation::FIXTURE.get(), 7);
    assert_eq!(Generation::new(9).get(), 9);
    Ok(())
}
