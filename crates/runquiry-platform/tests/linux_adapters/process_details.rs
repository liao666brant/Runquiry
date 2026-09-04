use super::*;
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
    let details = ProcessDetailsProvider::details(&platform, &identity)?
        .data
        .ok_or("详情应保留数据")?;
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
fn linux_details_preserves_partial_data_and_reports_each_missing_field() -> TestResult {
    let temp = TempProc::new("details-partial")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 100, "fxt-daemon")?;
    temp.write("100/environ", "FXT_MODE=synthetic\0malformed\0")?;
    let platform = platform_for(&temp)?;

    let identity = ProcessIdentity::new(Pid::new(100)?, Some(expected_start_time()), None);
    let inspected = ProcessDetailsProvider::details(&platform, &identity)?;
    let details = inspected
        .data()
        .ok_or("可读取 environ 时详情不得整体失败")?;
    assert_eq!(details.environment.len(), 1);
    assert!(
        inspected.has_issues(),
        "缺失 cwd/statm/io/fd/limits 必须可见"
    );
    assert!(inspected.issues.iter().any(|issue| {
        issue.code() == DiagnosticCode::ParseFailed && issue.message().contains("environ")
    }));
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
    let details = ProcessDetailsProvider::details(&platform, &identity)?
        .data
        .ok_or("详情应保留数据")?;
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
    assert!(
        matches!(
            result,
            Err(runquiry_core::InspectError::ProcessChanged { .. })
        ),
        "复用身份必须被拒绝"
    );

    // 进程消失 → 拒绝返回数据。
    let missing = ProcessIdentity::new(Pid::new(424_242)?, None, None);
    let result = ProcessDetailsProvider::details(&platform, &missing);
    assert!(matches!(
        result,
        Err(runquiry_core::InspectError::NotFound { .. })
    ));
    Ok(())
}
