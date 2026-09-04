//! Socket fixture 的三平台归因与输入边界。

use runquiry_core::{Protocol, SocketEntry};

use crate::support::fixtures::{load_sockets, validate_socket_entries};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn sockets_normal_fixture_passes_boundary_validation_for_all_platforms() -> TestResult {
    let linux = load_sockets("linux/sockets-normal.json")?;
    let entries = linux
        .inspection
        .data
        .as_deref()
        .ok_or_else(|| String::from("linux socket 快照应有数据"))?;
    assert_eq!(entries.len(), 3);
    assert!(entries.iter().all(|entry| entry.inode.is_some()));
    let macos = load_sockets("macos/sockets-normal.json")?;
    let entries = macos
        .inspection
        .data
        .as_deref()
        .ok_or_else(|| String::from("macos socket 快照应有数据"))?;
    assert!(entries.iter().all(|entry| entry.inode.is_none()));
    let unix = entries
        .iter()
        .find(|entry| entry.protocol == Protocol::Unix)
        .ok_or_else(|| String::from("macos 应含 Unix socket 条目"))?;
    assert_eq!(unix.port, None);
    assert_eq!(unix.address, "/opt/runquiry-fixtures/var/fxt-daemon.sock");
    let windows = load_sockets("windows/sockets-normal.json")?;
    let entries = windows
        .inspection
        .data
        .as_deref()
        .ok_or_else(|| String::from("windows socket 快照应有数据"))?;
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|entry| entry.inode.is_none()));
    Ok(())
}

#[test]
fn sockets_malformed_fixtures_are_rejected_by_loader() -> TestResult {
    let linux = load_sockets("linux/sockets-malformed.json")
        .err()
        .ok_or_else(|| String::from("port=0 必须被拒绝"))?;
    assert!(linux.contains("解析"));
    let macos = load_sockets("macos/sockets-malformed.json")
        .err()
        .ok_or_else(|| String::from("截断 JSON 必须被拒绝"))?;
    assert!(macos.contains("解析"));
    let windows = load_sockets("windows/sockets-malformed.json")
        .err()
        .ok_or_else(|| String::from("Unix socket 携带端口必须被拒绝"))?;
    assert!(windows.contains("Unix socket 条目"));
    Ok(())
}

#[test]
fn socket_boundary_rule_rejects_inline_violations() -> TestResult {
    let parse = |text: &str| -> Result<Vec<SocketEntry>, Box<dyn std::error::Error>> {
        let entries: Vec<SocketEntry> = serde_json::from_str(text)?;
        validate_socket_entries(&entries)?;
        Ok(entries)
    };
    let tcp = parse(
        r#"[{"inode":null,"address":"0.0.0.0","state":"LISTEN","protocol":"Tcp","owner_pid":null}]"#,
    );
    assert!(tcp.is_err());
    let unix_with_port = parse(
        r#"[{"inode":null,"port":8443,"address":"/opt/runquiry-fixtures/var/fxt.sock","state":"LISTEN","protocol":"Unix","owner_pid":null}]"#,
    );
    assert!(unix_with_port.is_err());
    let unix_without_port = parse(
        r#"[{"inode":null,"port":null,"address":"/opt/runquiry-fixtures/var/fxt.sock","state":"LISTEN","protocol":"Unix","owner_pid":null}]"#,
    )?;
    assert_eq!(unix_without_port[0].port, None);
    Ok(())
}
