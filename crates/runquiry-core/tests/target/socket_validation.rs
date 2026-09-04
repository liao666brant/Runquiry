//! `SocketEntry` 输入边界契约。

use runquiry_core::{Port, Protocol, SocketEntry, validate_socket_entry};

use super::TestResult;
fn tcp_entry(port: Option<u16>) -> SocketEntry {
    SocketEntry {
        inode: None,
        port: port.and_then(|p| Port::new(p).ok()),
        address: String::from("0.0.0.0"),
        remote_addr: None,
        state: String::from("LISTEN"),
        protocol: Protocol::Tcp,
        owner_pid: None,
    }
}

#[test]
fn target_socket_validation_accepts_protocol_port_pairs() {
    assert!(validate_socket_entry(&tcp_entry(Some(8443))).is_ok());
    let unix_ok = SocketEntry {
        inode: None,
        port: None,
        address: String::from("/opt/runquiry-fixtures/var/fxt.sock"),
        remote_addr: None,
        state: String::from("LISTEN"),
        protocol: Protocol::Unix,
        owner_pid: None,
    };
    assert!(validate_socket_entry(&unix_ok).is_ok());
}

#[test]
fn target_socket_validation_rejects_protocol_port_violations() -> TestResult {
    // Unix socket 不得携带端口。
    let unix_with_port = SocketEntry {
        port: Port::new(8443).ok(),
        protocol: Protocol::Unix,
        address: String::from("/opt/runquiry-fixtures/var/fxt.sock"),
        inode: None,
        remote_addr: None,
        state: String::from("LISTEN"),
        owner_pid: None,
    };
    let err = validate_socket_entry(&unix_with_port)
        .err()
        .ok_or_else(|| String::from("Unix socket 携带端口必须被拒绝"))?;
    assert!(
        err.contains("Unix socket 条目"),
        "错误须沿用 fixture 边界规则文案：{err}"
    );

    // TCP 条目缺少端口必须被拒绝。
    let tcp_without_port = tcp_entry(None);
    assert!(
        validate_socket_entry(&tcp_without_port).is_err(),
        "TCP 条目端口必须为 Some"
    );
    Ok(())
}

#[test]
fn target_socket_validation_covers_every_protocol() {
    let entry = |protocol: Protocol| SocketEntry {
        inode: None,
        port: Some(Port::new(80).unwrap_or(Port::MIN)),
        address: String::from("0.0.0.0"),
        remote_addr: None,
        state: String::from("LISTEN"),
        protocol,
        owner_pid: None,
    };
    for protocol in [Protocol::Tcp, Protocol::Tcp6, Protocol::Udp, Protocol::Udp6] {
        assert!(validate_socket_entry(&entry(protocol)).is_ok());
    }
    assert!(validate_socket_entry(&entry(Protocol::Unix)).is_err());
}
