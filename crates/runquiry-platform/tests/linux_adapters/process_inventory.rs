use super::*;
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
    let corrupted = listed
        .issues
        .iter()
        .filter(|issue| issue.message().contains("PID 103") || issue.message().contains("PID 104"))
        .count();
    assert_eq!(corrupted, 2);
    assert!(
        listed
            .issues
            .iter()
            .all(|issue| issue.code() == DiagnosticCode::Unknown)
    );
    Ok(())
}

#[test]
fn linux_process_baseline_reports_field_io_without_rejecting_empty_cmdline() -> TestResult {
    let temp = TempProc::new("field-io")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    temp.write("100/stat", &stat_content(100, "fxt", 'S', 1, 100))?;
    temp.write("100/cmdline", "")?;
    temp.symlink("100/exe", "/usr/bin/fxt")?;
    temp.write("100/cgroup", "0::/\n")?;
    let platform = platform_for(&temp)?;

    let listed = ProcessInventory::list(&platform);
    let process = listed
        .data()
        .and_then(|entries| entries.first())
        .ok_or("缺失 status 不得抹掉已取得的进程数据")?;
    assert_eq!(process.command_line, None, "合法空 cmdline 不是读取错误");
    assert_eq!(listed.issues.len(), 1, "只有缺失 status 应产生诊断");
    assert_eq!(listed.issues[0].code(), DiagnosticCode::Unknown);
    Ok(())
}
