//! 公共领域类型的边界、错误码与序列化契约测试（A3 验收 1-7）。

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use runquiry_core::{
    CapabilityStatus, ContainerKey, DiagnosticCode, DiagnosticIssue, InspectError, Inspection, Pid,
    Port, ProcessAction, ProcessIdentity, Protocol, QueryTarget, Renice, SocketEntry,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn pid_rejects_zero() {
    assert!(Pid::new(0).is_err());
    assert_eq!(Pid::new(1).map(Pid::get), Ok(1));
    assert_eq!(Pid::new(u32::MAX).map(Pid::get), Ok(u32::MAX));
}

#[test]
fn port_must_be_between_one_and_max() {
    // parity：invalid port must be between 1 and 65535
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
fn capability_status_has_four_distinguishable_states() {
    let supported = CapabilityStatus::Supported;
    let partial = CapabilityStatus::Partial(String::from("仅支持 TCP"));
    let unsupported = CapabilityStatus::Unsupported(String::from("Windows 无文件锁"));
    let unavailable = CapabilityStatus::Unavailable(String::from("lsof 缺失"));

    assert_ne!(supported, partial);
    assert_ne!(supported, unsupported);
    assert_ne!(supported, unavailable);
    assert_ne!(partial, unsupported);
    assert_ne!(partial, unavailable);
    assert_ne!(unsupported, unavailable);

    assert!(supported.is_fully_supported());
    assert!(!partial.is_fully_supported());
    assert!(!unsupported.is_fully_supported());
    assert!(!unavailable.is_fully_supported());

    assert_eq!(partial.reason(), Some("仅支持 TCP"));
    assert_eq!(supported.reason(), None);
}

#[test]
fn inspection_keeps_data_alongside_issues() {
    let inspection: Inspection<Vec<u32>> = Inspection::partial(
        vec![1, 2, 3],
        vec![DiagnosticIssue::new(
            DiagnosticCode::PermissionDenied,
            String::from("无法读取部分条目"),
        )],
    );

    assert_eq!(inspection.data, Some(vec![1, 2, 3]));
    assert_eq!(inspection.issues.len(), 1);
    assert!(inspection.has_issues());
    assert!(!inspection.is_empty());

    let failed: Inspection<Vec<u32>> = Inspection::failed(vec![DiagnosticIssue::new(
        DiagnosticCode::Unsupported,
        String::from("平台不支持"),
    )]);
    assert_eq!(failed.data, None);
    assert!(failed.is_empty());
}

#[test]
fn inspection_serializes_with_captured_at() -> TestResult {
    let inspection = Inspection::complete(42_u32);
    let text = serde_json::to_string(&inspection)?;
    assert!(text.contains("captured_at"));
    let back: Inspection<u32> = serde_json::from_str(&text)?;
    assert_eq!(back.data, Some(42));
    Ok(())
}

#[test]
fn inspect_error_stable_codes_are_asserted_one_by_one() -> TestResult {
    let pid = Pid::new(7)?;
    let error_cases = [
        (
            InspectError::InvalidTarget {
                reason: String::from("pid=0"),
            },
            "invalid_target",
        ),
        (
            InspectError::NotFound {
                subject: String::from("nginx"),
            },
            "not_found",
        ),
        (
            InspectError::Ambiguous {
                subject: String::from("node"),
                candidate_count: 3,
            },
            "ambiguous",
        ),
        (
            InspectError::PermissionDenied {
                subject: String::from("/proc/1/environ"),
            },
            "permission_denied",
        ),
        (
            InspectError::Unsupported {
                reason: String::from("Windows 文件锁"),
            },
            "unsupported",
        ),
        (
            InspectError::ExternalTool {
                program: String::from("lsof"),
                detail: String::from("exit status 1"),
            },
            "external_tool",
        ),
        (
            InspectError::ProcessChanged {
                identity: ProcessIdentity::new(pid, None, None),
            },
            "process_changed",
        ),
    ];

    for (error, code) in error_cases {
        assert_eq!(error.code(), code, "错误码必须逐项稳定");
        assert!(!error.to_string().is_empty(), "Display 面向用户，不得为空");
    }
    Ok(())
}

#[test]
fn process_identity_change_semantics() -> TestResult {
    let started = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000);
    let exe = PathBuf::from("/usr/bin/nginx");
    let other_exe = PathBuf::from("/usr/sbin/nginx");
    let pid = Pid::new(4242)?;
    let first = ProcessIdentity::new(pid, Some(started), Some(exe.clone()));
    let same = ProcessIdentity::new(pid, Some(started), Some(exe.clone()));
    let reused = ProcessIdentity::new(
        pid,
        Some(started + Duration::from_secs(5)),
        Some(exe.clone()),
    );
    let renamed = ProcessIdentity::new(pid, Some(started), Some(other_exe));
    let unknown_first = ProcessIdentity::new(pid, None, Some(exe.clone()));
    let unknown_second = ProcessIdentity::new(pid, None, Some(exe));

    assert!(
        first.same_process(&same),
        "同 PID 同 start_time 视为同一身份"
    );
    assert_eq!(first.pid(), reused.pid(), "PID 数值相同");
    assert!(
        !first.same_process(&reused),
        "同 PID 不同 start_time 表示 PID 被复用"
    );
    assert_ne!(first.start_time(), reused.start_time());
    assert!(
        first.same_process(&renamed),
        "executable 只是展示信息，不参与身份判定"
    );
    assert!(
        !first.same_process(&unknown_first) && !unknown_first.same_process(&first),
        "任一侧 start_time 缺失即不可验证"
    );
    assert!(
        !unknown_first.same_process(&unknown_second),
        "两侧 start_time 均为 None 也不得判定为同一进程（防止 PID 复用防护静默失效）"
    );
    Ok(())
}

#[test]
fn fixture_roundtrip_for_public_types() -> TestResult {
    let target = QueryTarget::ProcessName {
        query: String::from("nginx"),
        exact: true,
    };
    let text = serde_json::to_string(&target)?;
    let back: QueryTarget = serde_json::from_str(&text)?;
    assert_eq!(target, back);

    let port_target = QueryTarget::Port(Port::new(8080)?);
    let text = serde_json::to_string(&port_target)?;
    let back: QueryTarget = serde_json::from_str(&text)?;
    assert_eq!(port_target, back);

    let container = ContainerKey {
        runtime: String::from("docker"),
        id: String::from("abc123"),
    };
    assert_eq!(container.dedup_key(), "docker|abc123");
    let text = serde_json::to_string(&container)?;
    let back: ContainerKey = serde_json::from_str(&text)?;
    assert_eq!(container, back);

    let action = ProcessAction::Renice(Renice::try_from(5)?);
    let text = serde_json::to_string(&action)?;
    let back: ProcessAction = serde_json::from_str(&text)?;
    assert_eq!(action, back);

    let identity = ProcessIdentity::new(Pid::new(9)?, Some(SystemTime::UNIX_EPOCH), None);
    let text = serde_json::to_string(&identity)?;
    let back: ProcessIdentity = serde_json::from_str(&text)?;
    assert!(back.same_process(&identity), "序列化往返不得丢失身份");
    Ok(())
}

#[test]
fn socket_entry_covers_unix_protocol_and_remote_addr() -> TestResult {
    let port = Port::new(8080)?;
    let pid = Pid::new(31)?;

    // TCP 已知属主连接：携带对端地址（parity：SocketInfo.RemoteAddr）。
    let established = SocketEntry {
        inode: Some(12),
        port: Some(port),
        address: String::from("127.0.0.1"),
        remote_addr: Some(String::from("10.0.0.8")),
        state: String::from("ESTAB"),
        protocol: Protocol::Tcp,
        owner_pid: Some(pid),
    };
    let text = serde_json::to_string(&established)?;
    assert!(
        text.contains("remote_addr"),
        "remote_addr 必须进入序列化结果"
    );
    let back: SocketEntry = serde_json::from_str(&text)?;
    assert_eq!(back.remote_addr.as_deref(), Some("10.0.0.8"));
    assert_eq!(back.protocol, Protocol::Tcp);
    // 端口语义的往返不得被 serde(default) 吞掉：TCP 条目漏写 port 应在
    // fixture 校验中暴露，而不是静默读为 None。
    assert_eq!(back.port, Some(port));

    // 监听 socket：无对端，remote_addr 为 None。
    let listening = SocketEntry {
        inode: Some(13),
        port: Some(port),
        address: String::from("0.0.0.0"),
        remote_addr: None,
        state: String::from("LISTEN"),
        protocol: Protocol::Tcp,
        owner_pid: None,
    };
    let text = serde_json::to_string(&listening)?;
    let back: SocketEntry = serde_json::from_str(&text)?;
    assert_eq!(back.remote_addr, None);

    // 旧条目缺少 remote_addr 字段时按 None 读取，不因新增字段而拒载。
    let legacy: SocketEntry = serde_json::from_str(
        r#"{"inode":null,"port":8080,"address":"0.0.0.0","state":"LISTEN","protocol":"Tcp","owner_pid":null}"#,
    )?;
    assert_eq!(legacy.remote_addr, None);

    // Unix socket：无端口语义（/proc/net/unix 的 port 恒为 0，被 Port 拒绝），
    // port 必须为 None 而不是编造假端口；地址承载 socket 路径。
    let unix_socket = SocketEntry {
        inode: None,
        port: None,
        address: String::from("/run/docker.sock"),
        remote_addr: None,
        state: String::from("LISTEN"),
        protocol: Protocol::Unix,
        owner_pid: Some(pid),
    };
    let text = serde_json::to_string(&unix_socket)?;
    assert!(text.contains("\"Unix\""), "Protocol::Unix 必须可序列化");
    let back: SocketEntry = serde_json::from_str(&text)?;
    assert_eq!(back.protocol, Protocol::Unix);
    assert_eq!(back.address, "/run/docker.sock");
    assert_eq!(back.port, None, "Unix socket 不得携带编造的端口");

    // 缺少 port 字段的旧条目按 None 读取（兼容 serde(default)）。
    let no_port: SocketEntry = serde_json::from_str(
        r#"{"inode":null,"address":"/run/docker.sock","state":"LISTEN","protocol":"Unix","owner_pid":null}"#,
    )?;
    assert_eq!(no_port.port, None);
    Ok(())
}
