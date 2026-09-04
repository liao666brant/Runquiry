use super::*;
#[test]
fn linux_network_tables_parse_with_inode_attribution() -> TestResult {
    let temp = TempProc::new("network")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 100, "fxt-server")?;
    fs::create_dir_all(temp.pid_dir(100).join("fd"))?;
    temp.symlink("100/fd/3", "socket:[12345]")?;
    temp.symlink("100/fd/4", "socket:[4001]")?;
    // 端口 0000（非法）行：跳过并记 ParseFailed。
    temp.write(
        "net/tcp",
        "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
         0: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 12345 1 ffff8 100 0 0 10 0\n\
         1: 0A000061:0000 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 4444 1 ffff8 100 0 0 10 0\n",
    )?;
    // IPv6 回环 [::1]:9090（每 4 字节组小端序）。
    temp.write(
        "net/tcp6",
        "  sl  local_address remote_address                         st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
         0: 00000000000000000000000001000000:2382 00000000000000000000000000000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 20001 1 ffff8 100 0 0 10 0\n",
    )?;
    temp.write(
        "net/udp",
        "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode ref pointer drops\n\
         0: 00000000:14E9 00000000:0000 07 00000000:00000000 00:00000000 00000000     0        0 30001 1 ffff8 0 0 0 0 0\n",
    )?;
    temp.write(
        "net/unix",
        "Num       RefCount Protocol Flags    Type St Inode Path\n\
         0000000000000000:00000002 00000002 00000000 00010000 0001 03 4001 /run/fxt.sock\n",
    )?;
    let platform = platform_for(&temp)?;

    // open_ports：FD inode 归因 + 无主条目保留为 None + 非法端口跳过记诊断。
    let ports = NetworkInventory::open_ports(&platform);
    let entries = ports
        .data
        .as_deref()
        .ok_or_else(|| String::from("端口采集不应整体失败"))?;
    let attributed = entries
        .iter()
        .find(|entry| entry.pid.map(Pid::get) == Some(100))
        .ok_or_else(|| String::from("缺少归因条目"))?;
    assert_eq!(attributed.port.get(), 8080);
    assert_eq!(attributed.address, "127.0.0.1");
    assert_eq!(attributed.protocol, runquiry_core::Protocol::Tcp);
    assert_eq!(attributed.state, "LISTEN");
    assert!(
        entries
            .iter()
            .any(|entry| entry.pid.is_none() && entry.port.get() == 9090),
        "无主端口必须保留为 pid None"
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.pid.is_none() && entry.port.get() == 5353)
    );
    assert_eq!(ports.issues.len(), 1, "非法端口行必须记 ParseFailed");
    assert_eq!(ports.issues[0].code(), DiagnosticCode::ParseFailed);
    assert!(!entries.iter().any(|entry| entry.port.get() == 0));

    // sockets_of：TCP 归因 + Unix 条目无端口。
    let sockets = NetworkInventory::sockets_of(&platform, Pid::new(100)?);
    let list = sockets
        .data
        .as_deref()
        .ok_or_else(|| String::from("socket 采集不应整体失败"))?;
    let tcp = list
        .iter()
        .find(|entry| entry.protocol == runquiry_core::Protocol::Tcp)
        .ok_or_else(|| String::from("缺少 TCP 条目"))?;
    assert_eq!(tcp.port.map(runquiry_core::Port::get), Some(8080));
    assert_eq!(tcp.inode, Some(12_345));
    assert_eq!(tcp.owner_pid, Some(Pid::new(100)?));
    let unix = list
        .iter()
        .find(|entry| entry.protocol == runquiry_core::Protocol::Unix)
        .ok_or_else(|| String::from("缺少 Unix 条目"))?;
    assert_eq!(unix.port, None, "Unix socket 不得编造假端口");
    assert_eq!(unix.address, "/run/fxt.sock");
    assert_eq!(unix.state, "CONNECTED");
    Ok(())
}

#[test]
fn linux_network_fd_permission_yields_owner_none_with_diagnostic() -> TestResult {
    let temp = TempProc::new("fdperm")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 100, "fxt-server")?;
    write_basic_process(&temp, 105, "fxt-restricted")?;
    fs::create_dir_all(temp.pid_dir(105).join("fd"))?;
    temp.write(
        "net/tcp",
        "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
         0: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 12345 1 ffff8 100 0 0 10 0\n",
    )?;
    let platform = platform_for(&temp)?;

    // 直接以受限 fd 目录探测（root 环境跳过）。
    let restricted = temp.pid_dir(105).join("fd");
    fs::set_permissions(&restricted, fs::Permissions::from_mode(0o000))?;
    let permission_enforced = fs::read_dir(&restricted).is_err();

    let ports = NetworkInventory::open_ports(&platform);
    let entries = ports
        .data
        .as_deref()
        .ok_or_else(|| String::from("端口采集不应整体失败"))?;
    // 条目保留（不静默丢条目），无归因时 pid None。
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].pid, None);
    if permission_enforced {
        assert_eq!(ports.issues[0].code(), DiagnosticCode::PermissionDenied);
    }
    // 还原权限便于 tempdir 清理。
    fs::set_permissions(&restricted, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

#[test]
fn linux_network_unreadable_tables_do_not_fake_success() -> TestResult {
    let temp = TempProc::new("netperm")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    // 无任何 net 表：不得以空集合冒充成功。
    let platform = platform_for(&temp)?;
    let ports = NetworkInventory::open_ports(&platform);
    assert!(ports.is_empty());
    assert_eq!(ports.issues[0].code(), DiagnosticCode::Unknown);

    // 权限失败（非 ENOENT）：整体失败并带 PermissionDenied。
    let temp2 = TempProc::new("netperm2")?;
    temp2.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    temp2.write("net/tcp", "  sl  local_address rem_address   st tx_queue\n")?;
    let restricted = temp2.root.join("net/tcp");
    fs::set_permissions(&restricted, fs::Permissions::from_mode(0o000))?;
    let permission_enforced = fs::read(&restricted).is_err();
    if permission_enforced {
        let platform = platform_for(&temp2)?;
        let ports = NetworkInventory::open_ports(&platform);
        assert!(ports.is_empty());
        assert_eq!(ports.issues[0].code(), DiagnosticCode::PermissionDenied);
        fs::set_permissions(&restricted, fs::Permissions::from_mode(0o644))?;
    }
    Ok(())
}
