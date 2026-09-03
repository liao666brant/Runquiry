//! fixture 消费测试（A5 任务 1/2/6/7）：三平台 × 八类场景的装载、
//! 部分成功语义、`SocketEntry` 输入边界校验与确定性基建行为。
//!
//! 所有 fixture 位于 workspace 根 `tests/fixtures/`，数据全部为合成值；
//! 场景 → 文件 → 测试的映射见 `tests/fixtures/README.md`。

mod support;

use std::time::{Duration, SystemTime};

use runquiry_core::{
    CapabilityStatus, ContainerSummary, FileLockEntry, OpenPortEntry, Pid, ProcessIdentity,
    ProcessSummary, Protocol, SocketEntry,
};
use support::fixtures::{
    LoadedFixture, fixtures_root, load, load_sockets, validate_socket_entries,
};
use support::{FixedClock, Generation};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const PLATFORMS: [&str; 3] = ["linux", "macos", "windows"];
const CAPTURED_AT_MS: u64 = 1_700_000_000_000;

fn expected_captured_at() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_millis(CAPTURED_AT_MS)
}

/// 断言封套元数据：平台/场景/固定 generation/确定性采集时刻。
fn assert_metadata<T>(fixture: &LoadedFixture<T>, platform: &str, scenario: &str) {
    assert_eq!(fixture.platform, platform, "平台名必须与目录一致");
    assert_eq!(fixture.scenario, scenario, "场景名必须与文件名对应");
    assert_eq!(
        fixture.generation,
        Generation::FIXTURE.get(),
        "generation 必须固定为 7"
    );
    assert_eq!(
        fixture.captured_at,
        expected_captured_at(),
        "采集时刻必须确定性"
    );
}

#[test]
fn processes_normal_fixture_loads_for_all_platforms() -> TestResult {
    for platform in PLATFORMS {
        let f = load::<Vec<ProcessSummary>>(&format!("{platform}/processes-normal.json"))?;
        assert_metadata(&f, platform, "normal");
        let entries = f
            .inspection
            .data
            .as_deref()
            .ok_or_else(|| String::from("normal 场景必须携带数据"))?;
        assert!(!entries.is_empty());
        assert!(f.inspection.issues.is_empty(), "normal 场景不得携带诊断");
        for entry in entries {
            assert!(
                entry.command.starts_with("fxt-"),
                "进程名必须是合成值：{}",
                entry.command
            );
            assert!(entry.identity.pid().get() > 0);
        }
    }
    Ok(())
}

#[test]
fn processes_empty_fixture_yields_complete_empty_data() -> TestResult {
    for platform in PLATFORMS {
        let f = load::<Vec<ProcessSummary>>(&format!("{platform}/processes-empty.json"))?;
        assert_metadata(&f, platform, "empty");
        assert_eq!(
            f.inspection.data.as_deref().map(<[ProcessSummary]>::len),
            Some(0),
            "空结果不是失败"
        );
        assert!(f.inspection.issues.is_empty());
        assert!(!f.inspection.is_empty(), "data = Some(空) 不是完全失败");
    }
    Ok(())
}

#[test]
fn processes_partial_fixture_keeps_data_alongside_permission_issue() -> TestResult {
    for platform in PLATFORMS {
        let f = load::<Vec<ProcessSummary>>(&format!("{platform}/processes-partial.json"))?;
        assert_metadata(&f, platform, "partial");
        let entries = f
            .inspection
            .data
            .as_deref()
            .ok_or_else(|| String::from("partial 场景必须保留已取得数据"))?;
        assert_eq!(entries.len(), 1, "部分成功不得丢弃其余条目");
        assert_eq!(f.inspection.issues.len(), 1);
        assert_eq!(
            f.inspection.issues[0].code().code(),
            "permission_denied",
            "部分成功场景使用权限类诊断"
        );
        assert!(f.inspection.has_issues());
    }
    Ok(())
}

#[test]
fn processes_permission_fixture_fails_without_data() -> TestResult {
    for platform in PLATFORMS {
        let f = load::<Vec<ProcessSummary>>(&format!("{platform}/processes-permission.json"))?;
        assert_metadata(&f, platform, "permission");
        assert!(f.inspection.is_empty(), "权限整体失败时不得有数据");
        assert_eq!(f.inspection.issues[0].code().code(), "permission_denied");
    }
    Ok(())
}

#[test]
fn containers_tool_missing_fixture_reports_external_tool_and_capability() -> TestResult {
    // Linux：docker 可用 + podman 缺失 → 部分成功（单运行时失败不阻断其他运行时）。
    let linux = load::<Vec<ContainerSummary>>("linux/containers-tool-missing.json")?;
    let linux_entries = linux
        .inspection
        .data
        .as_deref()
        .ok_or_else(|| String::from("linux 容器场景必须保留 docker 数据"))?;
    assert_eq!(linux_entries.len(), 1);
    assert_eq!(linux_entries[0].key.runtime, "docker");
    assert_eq!(
        linux.inspection.issues[0].code().code(),
        "external_tool_failed"
    );
    assert!(matches!(
        linux.capability,
        Some(CapabilityStatus::Partial(_))
    ));

    // macOS / Windows：唯一运行时缺失 → 完全失败 + Unavailable 能力。
    for platform in ["macos", "windows"] {
        let f = load::<Vec<ContainerSummary>>(&format!("{platform}/containers-tool-missing.json"))?;
        assert!(f.inspection.is_empty(), "{platform} 工具缺失时不得有数据");
        assert_eq!(f.inspection.issues[0].code().code(), "external_tool_failed");
        assert_eq!(
            f.capability,
            Some(CapabilityStatus::Unavailable(String::from(
                "容器运行时 CLI 未安装（合成场景）"
            ))),
            "运行时缺失是能力状态，不是空集合"
        );
    }
    Ok(())
}

#[test]
fn open_ports_timeout_fixture_reports_timeout() -> TestResult {
    // macOS：lsof 超时但返回已取得的端口 → 部分成功。
    let macos = load::<Vec<OpenPortEntry>>("macos/open-ports-timeout.json")?;
    let macos_ports = macos
        .inspection
        .data
        .as_deref()
        .ok_or_else(|| String::from("macos 超时场景保留已取得端口"))?;
    assert_eq!(macos_ports.len(), 1);
    assert_eq!(macos_ports[0].port.get(), 8443);
    assert_eq!(macos.inspection.issues[0].code().code(), "timeout");

    // Linux / Windows：整体超时 → 无数据。
    for platform in ["linux", "windows"] {
        let f = load::<Vec<OpenPortEntry>>(&format!("{platform}/open-ports-timeout.json"))?;
        assert!(f.inspection.is_empty());
        assert_eq!(f.inspection.issues[0].code().code(), "timeout");
    }
    Ok(())
}

#[test]
fn sockets_normal_fixture_passes_boundary_validation_for_all_platforms() -> TestResult {
    let linux = load_sockets("linux/sockets-normal.json")?;
    let linux_entries = linux
        .inspection
        .data
        .as_deref()
        .ok_or_else(|| String::from("linux socket 快照应有数据"))?;
    assert_eq!(linux_entries.len(), 3, "Tcp/Tcp6/Udp 三种协议");
    assert!(
        linux_entries.iter().all(|e| e.inode.is_some()),
        "Linux 条目携带 inode"
    );

    let macos = load_sockets("macos/sockets-normal.json")?;
    let macos_entries = macos
        .inspection
        .data
        .as_deref()
        .ok_or_else(|| String::from("macos socket 快照应有数据"))?;
    assert!(
        macos_entries.iter().all(|e| e.inode.is_none()),
        "macOS 条目 inode 为 None"
    );
    let unix = macos_entries
        .iter()
        .find(|e| e.protocol == Protocol::Unix)
        .ok_or_else(|| String::from("macos 应含 Unix socket 条目"))?;
    assert_eq!(unix.port, None, "Unix socket 不得携带端口");
    assert_eq!(unix.address, "/opt/runquiry-fixtures/var/fxt-daemon.sock");

    let windows = load_sockets("windows/sockets-normal.json")?;
    let windows_entries = windows
        .inspection
        .data
        .as_deref()
        .ok_or_else(|| String::from("windows socket 快照应有数据"))?;
    assert_eq!(windows_entries.len(), 2);
    assert!(
        windows_entries.iter().all(|e| e.inode.is_none()),
        "Windows 条目 inode 为 None"
    );
    Ok(())
}

#[test]
fn sockets_malformed_fixtures_are_rejected_by_loader() -> TestResult {
    // Linux：JSON 合法但 TCP 条目 port = 0 → Port 反序列化拒绝（语义损坏）。
    let linux_err = load_sockets("linux/sockets-malformed.json")
        .err()
        .ok_or_else(|| String::from("port=0 必须被拒绝"))?;
    assert!(
        linux_err.contains("解析"),
        "port=0 应在反序列化层失败：{linux_err}"
    );

    // macOS：JSON 本身被截断 → 结构损坏，装载失败。
    let macos_err = load_sockets("macos/sockets-malformed.json")
        .err()
        .ok_or_else(|| String::from("截断 JSON 必须被拒绝"))?;
    assert!(
        macos_err.contains("解析"),
        "截断 JSON 应在解析层失败：{macos_err}"
    );

    // Windows：Unix socket 携带端口 → 边界校验规则拒绝。
    let windows_err = load_sockets("windows/sockets-malformed.json")
        .err()
        .ok_or_else(|| String::from("Unix socket 携带端口必须被拒绝"))?;
    assert!(
        windows_err.contains("Unix socket 条目"),
        "应命中端口配对规则：{windows_err}"
    );
    Ok(())
}

#[test]
fn pid_reuse_fixture_provides_distinguishable_identities() -> TestResult {
    for platform in PLATFORMS {
        let f = load::<Vec<ProcessIdentity>>(&format!("{platform}/pid-reuse.json"))?;
        assert_metadata(&f, platform, "pid_reuse");
        let identities = f
            .inspection
            .data
            .as_deref()
            .ok_or_else(|| String::from("pid_reuse 场景应有身份快照"))?;
        assert_eq!(identities.len(), 3, "原身份 / 复用身份 / start_time 缺失");

        let (original, reused, unverifiable) = (&identities[0], &identities[1], &identities[2]);
        assert_eq!(original.pid(), reused.pid(), "PID 数值相同");
        assert!(
            !original.same_process(reused),
            "同 PID 不同 start_time 表示 PID 被复用"
        );
        assert!(
            !original.same_process(unverifiable),
            "start_time 为 None 的身份不可验证"
        );
        assert_eq!(unverifiable.start_time(), None);
    }
    Ok(())
}

#[test]
fn socket_boundary_rule_rejects_inline_violations() -> TestResult {
    let parse = |text: &str| -> Result<Vec<SocketEntry>, Box<dyn std::error::Error>> {
        let entries: Vec<SocketEntry> = serde_json::from_str(text)?;
        validate_socket_entries(&entries)?;
        Ok(entries)
    };

    // TCP 条目缺少端口（serde(default) 会读成 None）→ 必须被规则拒绝。
    let tcp_without_port = parse(
        r#"[{"inode":null,"address":"0.0.0.0","state":"LISTEN","protocol":"Tcp","owner_pid":null}]"#,
    );
    assert!(tcp_without_port.is_err(), "TCP 条目端口必须为 Some");

    // Unix socket 携带端口 → 拒绝；不携带端口 → 接受。
    let unix_with_port = parse(
        r#"[{"inode":null,"port":8443,"address":"/opt/runquiry-fixtures/var/fxt.sock","state":"LISTEN","protocol":"Unix","owner_pid":null}]"#,
    );
    assert!(unix_with_port.is_err(), "Unix socket 不得携带端口");

    let unix_without_port = parse(
        r#"[{"inode":null,"port":null,"address":"/opt/runquiry-fixtures/var/fxt.sock","state":"LISTEN","protocol":"Unix","owner_pid":null}]"#,
    )?;
    assert_eq!(unix_without_port[0].port, None);
    Ok(())
}

#[test]
fn windows_file_locks_fixture_marks_capability_unsupported() -> TestResult {
    let f = load::<FileLockEntry>("windows/file-locks-unsupported.json")?;
    assert_metadata(&f, "windows", "capability_unsupported");
    assert!(f.inspection.is_empty(), "Unsupported 能力不得返回伪数据");
    assert!(
        f.inspection.issues.is_empty(),
        "Unsupported 是能力状态，不是故障"
    );
    assert_eq!(
        f.capability,
        Some(CapabilityStatus::Unsupported(String::from(
            "Windows 无文件锁采集（合成场景）"
        )))
    );
    Ok(())
}

#[test]
fn macos_file_locks_fixture_loads_lock_entries() -> TestResult {
    let f = load::<Vec<FileLockEntry>>("macos/file-locks-normal.json")?;
    let locks = f
        .inspection
        .data
        .as_deref()
        .ok_or_else(|| String::from("macos 文件锁快照应有数据"))?;
    assert_eq!(locks.len(), 1);
    assert_eq!(locks[0].pid, Pid::new(5150)?);
    assert_eq!(locks[0].lock_type, runquiry_core::LockType::Flock);
    assert_eq!(locks[0].mode, runquiry_core::LockMode::Write);
    assert_eq!(
        locks[0].path,
        std::path::PathBuf::from("/opt/runquiry-fixtures/var/fxt-daemon.lock")
    );
    Ok(())
}

#[test]
fn linux_file_locks_fixture_loads_lock_entries() -> TestResult {
    // parity §1：Linux 文件锁来源是 /proc/locks（POSIX 记录锁与 flock 均可能出现）。
    let f = load::<Vec<FileLockEntry>>("linux/file-locks-normal.json")?;
    assert_metadata(&f, "linux", "normal");
    let locks = f
        .inspection
        .data
        .as_deref()
        .ok_or_else(|| String::from("linux 文件锁快照应有数据"))?;
    assert_eq!(locks.len(), 2, "Flock + Posix 各一条");
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
    assert!(fixtures_root().join("macos").is_dir());
    assert!(fixtures_root().join("windows").is_dir());

    let mut clock = FixedClock::at_epoch_ms(CAPTURED_AT_MS)?;
    assert_eq!(clock.now(), expected_captured_at());
    clock.advance(Duration::from_secs(3));
    assert_eq!(clock.now(), expected_captured_at() + Duration::from_secs(3));
    assert_eq!(Generation::FIXTURE.get(), 7);
    assert_eq!(Generation::new(9).get(), 9);
    Ok(())
}
