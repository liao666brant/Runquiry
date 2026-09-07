use super::*;

#[test]
fn linux_file_inventory_bounds_high_cardinality_diagnostics_and_keeps_data() -> TestResult {
    use std::fmt::Write as _;

    // Given：一个成功 FD、一个失败 FD，以及十万条同类非法锁模式。
    let temp = TempProc::new("inventory-bounded-issues")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 100, "fxt-holder")?;
    let target = temp.root.join("fxt-data.bin");
    fs::write(&target, b"fxt")?;
    let inode = fs::metadata(&target)?.ino();
    fs::create_dir_all(temp.pid_dir(100).join("fd"))?;
    temp.symlink("100/fd/7", &target.to_string_lossy())?;
    temp.write("100/fd/8", "不是符号链接")?;
    let mut locks = String::with_capacity(8_000_000);
    for row in 0..100_000 {
        writeln!(locks, "{row}: POSIX ADVISORY WEIRD 100 08:01:{inode} 0 EOF")?;
    }
    temp.write("locks", &locks)?;
    let platform = platform_for(&temp)?;

    // When：读取全量 File Inventory。
    let inventory = FileInventory::list(&platform);

    // Then：成功 FD 保留，诊断数量受类别常数上界约束并汇总总数。
    let entries = inventory
        .data
        .as_deref()
        .ok_or_else(|| String::from("高基数局部失败不应抹掉成功数据"))?;
    assert!(entries.iter().any(|entry| entry.fd == Some(7)));
    assert!(
        inventory.issues.len() <= 8,
        "十万条同类失败不得生成逐行诊断，实际 {} 条",
        inventory.issues.len()
    );
    assert!(
        inventory
            .issues
            .iter()
            .any(|issue| issue.message().contains("100000")),
        "聚合诊断应报告同类失败总数"
    );
    Ok(())
}

#[test]
fn linux_locks_of_reports_missing_fd_directory_as_partial() -> TestResult {
    // Given：真实锁存在，但持有进程的 fd 目录不可读（此处确定性模拟为缺失）。
    let temp = TempProc::new("locks-of-missing-fd-dir")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 100, "fxt-holder")?;
    temp.write("locks", "1: POSIX ADVISORY WRITE 100 08:01:4242 0 EOF\n")?;
    let platform = platform_for(&temp)?;

    // When：按 PID 查询真实锁。
    let inspected = ProcessFileLocks::locks_of(&platform, Pid::new(100)?);

    // Then：锁数据保留，但结果必须是 Partial。
    assert_eq!(inspected.data.as_ref().map(Vec::len), Some(1));
    assert!(
        inspected
            .issues
            .iter()
            .any(|issue| issue.message().contains("fd 目录"))
    );
    Ok(())
}

#[test]
fn linux_locks_of_reports_unreadable_fd_link_as_partial() -> TestResult {
    // Given：fd 目录可枚举，但其中条目不是符号链接。
    let temp = TempProc::new("locks-of-bad-fd-link")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 100, "fxt-holder")?;
    temp.write("100/fd/5", "不是符号链接")?;
    temp.write("locks", "1: POSIX ADVISORY WRITE 100 08:01:4242 0 EOF\n")?;
    let platform = platform_for(&temp)?;

    // When：按 PID 查询真实锁。
    let inspected = ProcessFileLocks::locks_of(&platform, Pid::new(100)?);

    // Then：锁数据保留，readlink 失败可观察。
    assert_eq!(inspected.data.as_ref().map(Vec::len), Some(1));
    assert!(
        inspected
            .issues
            .iter()
            .any(|issue| issue.message().contains("fd 5"))
    );
    Ok(())
}

#[test]
fn linux_file_inventory_skips_zero_pid_lock_without_mapping_to_init() -> TestResult {
    // Given：外部锁表包含领域不允许的 PID 0。
    let temp = TempProc::new("locks-zero-pid")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    temp.write("locks", "1: POSIX ADVISORY WRITE 0 08:01:4242 0 EOF\n")?;
    let platform = platform_for(&temp)?;

    // When：读取全量 File Inventory。
    let inventory = FileInventory::list(&platform);

    // Then：非法锁被跳过，不得伪装为 PID 1，并给出解析诊断。
    assert!(inventory.data.as_deref().is_some_and(<[_]>::is_empty));
    assert!(
        inventory
            .issues
            .iter()
            .any(|issue| issue.code() == DiagnosticCode::ParseFailed)
    );
    Ok(())
}

#[test]
fn linux_file_inventory_lists_plain_fd_without_fabricating_lock() -> TestResult {
    // Given：一个进程只打开普通文件，没有 /proc/locks 记录。
    let temp = TempProc::new("inventory-plain")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 100, "fxt-holder")?;
    let target = temp.root.join("fxt-data.bin");
    fs::write(&target, b"fxt")?;
    fs::create_dir_all(temp.pid_dir(100).join("fd"))?;
    temp.symlink("100/fd/7", &target.to_string_lossy())?;
    temp.symlink("100/fd/8", "pipe:[4242]")?;
    temp.write("locks", "")?;
    let platform = platform_for(&temp)?;

    // When：读取全量 File Inventory。
    let inventory = FileInventory::list(&platform);

    // Then：FD 号真实可见，且没有伪造锁类型/模式。
    let entries = inventory
        .data
        .as_deref()
        .ok_or_else(|| String::from("文件清单不应整体失败"))?;
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].fd, Some(7));
    assert_eq!(entries[0].lock, None);
    assert_eq!(entries[1].path, Path::new("pipe:[4242]"));
    assert_eq!(entries[1].fd, Some(8));
    assert_eq!(entries[1].lock, None);
    Ok(())
}

#[test]
fn linux_file_inventory_keeps_plain_rows_when_lock_parse_is_partial() -> TestResult {
    // Given：普通 FD 可读，但锁表包含不可识别的锁模式。
    let temp = TempProc::new("inventory-partial-lock")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 100, "fxt-holder")?;
    let target = temp.root.join("fxt-data.bin");
    fs::write(&target, b"fxt")?;
    let inode = fs::metadata(&target)?.ino();
    fs::create_dir_all(temp.pid_dir(100).join("fd"))?;
    temp.symlink("100/fd/7", &target.to_string_lossy())?;
    temp.write(
        "locks",
        &format!("1: POSIX ADVISORY WEIRD 100 08:01:{inode} 0 EOF\n"),
    )?;
    let platform = platform_for(&temp)?;

    // When：读取全量 File Inventory。
    let inventory = FileInventory::list(&platform);

    // Then：保留普通 FD，并携带锁解析诊断。
    let entries = inventory
        .data
        .as_deref()
        .ok_or_else(|| String::from("部分锁解析失败不应抹掉普通 FD"))?;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].fd, Some(7));
    assert_eq!(entries[0].lock, None);
    assert!(
        inventory
            .issues
            .iter()
            .any(|issue| issue.code() == DiagnosticCode::ParseFailed)
    );
    Ok(())
}

#[test]
fn linux_file_inventory_keeps_other_processes_when_one_fd_directory_fails() -> TestResult {
    // Given：PID 100 有可读 FD，PID 101 的 fd 目录缺失。
    let temp = TempProc::new("inventory-partial-fd")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 100, "fxt-readable")?;
    write_basic_process(&temp, 101, "fxt-vanished")?;
    let target = temp.root.join("fxt-data.bin");
    fs::write(&target, b"fxt")?;
    fs::create_dir_all(temp.pid_dir(100).join("fd"))?;
    temp.symlink("100/fd/8", &target.to_string_lossy())?;
    temp.write("locks", "")?;
    let platform = platform_for(&temp)?;

    // When：读取全量 File Inventory。
    let inventory = FileInventory::list(&platform);

    // Then：PID 100 的数据保留，PID 101 仅贡献诊断。
    let entries = inventory
        .data
        .as_deref()
        .ok_or_else(|| String::from("单 PID 失败不应造成整体失败"))?;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].pid, Pid::new(100)?);
    assert!(
        inventory
            .issues
            .iter()
            .any(|issue| issue.message().contains("101") && issue.message().contains("fd"))
    );
    Ok(())
}
#[test]
fn linux_locks_parse_with_lock_priority_dedup() -> TestResult {
    let temp = TempProc::new("locks")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 100, "fxt-holder")?;
    write_basic_process(&temp, 101, "fxt-rw")?;
    // 真实文件（fd 链接目标可 stat，inode 匹配生效）。
    let target = temp.root.join("fxt-data.bin");
    fs::write(&target, b"fxt")?;
    let target_ino = fs::metadata(&target)?.ino();
    let second = temp.root.join("fxt-second.bin");
    fs::write(&second, b"fxt2")?;
    let second_ino = fs::metadata(&second)?.ino();
    fs::create_dir_all(temp.pid_dir(100).join("fd"))?;
    temp.symlink("100/fd/5", &format!("{}", target.display()))?;
    temp.symlink("100/fd/6", &format!("{}", target.display()))?;
    fs::create_dir_all(temp.pid_dir(101).join("fd"))?;
    temp.symlink("101/fd/5", &format!("{}", second.display()))?;
    temp.write(
        "locks",
        &format!(
            "1: FLOCK ADVISORY WRITE 100 08:01:{target_ino} 0 EOF\n\
             2: OFDLCK ADVISORY RW 101 08:01:{second_ino} 0 EOF\n\
             3: POSIX ADVISORY WEIRD 102 08:01:{target_ino} 0 0\n"
        ),
    )?;
    let platform = platform_for(&temp)?;

    let holders = FileInventory::holders(&platform, &target);
    let entries = holders
        .data
        .as_deref()
        .ok_or_else(|| String::from("锁采集不应整体失败"))?;
    // 锁记录优先：PID 100 对同一 path 的两个 fd 只产出一条锁记录。
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].pid, Pid::new(100)?);
    assert_eq!(entries[0].process, "fxt-holder");
    assert_eq!(entries[0].path, target);
    assert_eq!(
        entries[0].lock,
        Some(LockMetadata {
            lock_type: LockType::Flock,
            mode: LockMode::Write,
        })
    );
    // 未知模式行被跳过并记诊断。
    assert_eq!(holders.issues.len(), 1);
    assert_eq!(holders.issues[0].code(), DiagnosticCode::ParseFailed);

    // OFDLCK + RW。
    let holders2 = FileInventory::holders(&platform, &second);
    let entries2 = holders2
        .data
        .as_deref()
        .ok_or_else(|| String::from("锁采集不应整体失败"))?;
    assert_eq!(entries2.len(), 1);
    assert_eq!(
        entries2[0].lock,
        Some(LockMetadata {
            lock_type: LockType::Ofdlck,
            mode: LockMode::ReadWrite,
        })
    );
    Ok(())
}

#[test]
fn linux_plain_fd_holder_reported_without_duplicating_lock_entry() -> TestResult {
    let temp = TempProc::new("fddup")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 100, "fxt-holder")?;
    let target = temp.root.join("fxt-data.bin");
    fs::write(&target, b"fxt")?;
    let ino = fs::metadata(&target)?.ino();
    fs::create_dir_all(temp.pid_dir(100).join("fd"))?;
    temp.symlink("100/fd/5", &format!("{}", target.display()))?;
    // 只有普通 FD，没有锁记录：不得伪造锁元数据。
    temp.write("locks", "")?;
    let platform = platform_for(&temp)?;

    let holders = FileInventory::holders(&platform, &target);
    let entries = holders
        .data
        .as_deref()
        .ok_or_else(|| String::from("锁采集不应整体失败"))?;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].pid, Pid::new(100)?);
    assert_eq!(entries[0].fd, Some(5));
    assert_eq!(entries[0].lock, None);
    assert_eq!(holders.issues.len(), 0);

    // 锁记录 + 同 PID/path 普通 FD → 锁记录优先，不重复。
    temp.write(
        "locks",
        &format!("1: POSIX ADVISORY READ 100 08:01:{ino} 0 EOF\n"),
    )?;
    let holders = FileInventory::holders(&platform, &target);
    let entries = holders
        .data
        .as_deref()
        .ok_or_else(|| String::from("锁采集不应整体失败"))?;
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].lock,
        Some(LockMetadata {
            lock_type: LockType::Posix,
            mode: LockMode::Read,
        })
    );
    Ok(())
}

/// 进程级文件锁（`ProcessFileLocks`）：/proc/locks 按持有者 PID 过滤，
/// 与 `holders`（按路径）共享解析路径。
#[test]
fn linux_locks_of_process_filters_by_pid() -> TestResult {
    let temp = TempProc::new("locks-of")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 100, "fxt-holder")?;
    write_basic_process(&temp, 101, "fxt-other")?;
    let target = temp.root.join("fxt-data.bin");
    fs::write(&target, b"fxt")?;
    let target_ino = fs::metadata(&target)?.ino();
    let second = temp.root.join("fxt-second.bin");
    fs::write(&second, b"fxt2")?;
    let second_ino = fs::metadata(&second)?.ino();
    fs::create_dir_all(temp.pid_dir(100).join("fd"))?;
    temp.symlink("100/fd/5", &format!("{}", target.display()))?;
    fs::create_dir_all(temp.pid_dir(101).join("fd"))?;
    temp.symlink("101/fd/5", &format!("{}", second.display()))?;
    temp.write(
        "locks",
        &format!(
            "1: FLOCK ADVISORY WRITE 100 08:01:{target_ino} 0 EOF\n\
             2: OFDLCK ADVISORY RW 101 08:01:{second_ino} 0 EOF\n"
        ),
    )?;
    let platform = platform_for(&temp)?;

    let own = ProcessFileLocks::locks_of(&platform, Pid::new(100)?);
    let entries = own
        .data
        .as_deref()
        .ok_or_else(|| String::from("锁查询不应整体失败"))?;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].pid, Pid::new(100)?);
    assert_eq!(entries[0].lock_type, LockType::Flock);

    let other = ProcessFileLocks::locks_of(&platform, Pid::new(200)?);
    assert!(
        other
            .data
            .as_deref()
            .is_some_and(<[runquiry_core::FileLockEntry]>::is_empty),
        "无锁进程应得到空列表而非失败"
    );
    Ok(())
}
