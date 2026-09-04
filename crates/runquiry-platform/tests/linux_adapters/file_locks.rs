use super::*;
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
    assert_eq!(entries[0].lock_type, LockType::Flock);
    assert_eq!(entries[0].mode, LockMode::Write);
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
    assert_eq!(entries2[0].lock_type, LockType::Ofdlck);
    assert_eq!(entries2[0].mode, LockMode::ReadWrite);
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
    // 只有普通 FD，没有锁记录：以 Other + Read 表达「打开即持有」。
    temp.write("locks", "")?;
    let platform = platform_for(&temp)?;

    let holders = FileInventory::holders(&platform, &target);
    let entries = holders
        .data
        .as_deref()
        .ok_or_else(|| String::from("锁采集不应整体失败"))?;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].pid, Pid::new(100)?);
    assert_eq!(entries[0].lock_type, LockType::Other);
    assert_eq!(entries[0].mode, LockMode::Read);
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
    assert_eq!(entries[0].lock_type, LockType::Posix);
    assert_eq!(entries[0].mode, LockMode::Read);
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
