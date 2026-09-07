//! 按文件锁持有者解析的契约。

use std::path::PathBuf;

use runquiry_core::{FileInventoryEntry, Pid, Resolution, resolve_file_holders};

use super::TestResult;

/// 合成 PID 构造（测试值恒合法）。
fn pid_value(value: u32) -> Pid {
    Pid::new(value).unwrap_or(Pid::MIN)
}

#[test]
fn target_file_resolution_delegates_to_holders_results() -> TestResult {
    let path = PathBuf::from("/opt/runquiry-fixtures/var/fxt.log");
    let entry = |pid: u32| FileInventoryEntry {
        pid: pid_value(pid),
        process: String::from("fxt-daemon"),
        path: path.clone(),
        fd: Some(7),
        lock: None,
    };
    let err = resolve_file_holders(&[], &path)
        .err()
        .ok_or_else(|| String::from("空 holders 应报 not_found"))?;
    assert_eq!(err.code(), "not_found");
    assert!(
        err.to_string().contains("fxt.log"),
        "NotFound subject 须含路径：{err}"
    );

    assert_eq!(
        resolve_file_holders(&[entry(7)], &path)?,
        Resolution::Unique(Pid::new(7)?)
    );
    let Resolution::Ambiguous(pids) = resolve_file_holders(&[entry(9), entry(3)], &path)? else {
        return Err(String::from("多持有者应返回 Ambiguous").into());
    };
    assert_eq!(pids, vec![Pid::new(3)?, Pid::new(9)?], "去重升序");
    Ok(())
}
