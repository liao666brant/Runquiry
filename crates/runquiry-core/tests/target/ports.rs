//! 按端口解析进程的契约。

use runquiry_core::{
    OpenPortEntry, Pid, Port, Protocol, Resolution, SocketEntry, resolve_port_owner,
    resolve_port_owner_in_sockets,
};

use super::TestResult;
fn open_port(pid: Option<u32>, port: u16) -> OpenPortEntry {
    OpenPortEntry {
        pid: pid.and_then(|p| Pid::new(p).ok()),
        port: Port::new(port).unwrap_or(Port::MIN),
        address: String::from("0.0.0.0"),
        protocol: Protocol::Tcp,
        state: String::from("LISTEN"),
    }
}

/// 端口解析：唯一属主 / 多属主 / 属主不可知 / 无条目（parity §2 + 哨兵）。
#[test]
fn target_port_resolution_maps_entries_to_owner_resolution() -> TestResult {
    let port = Port::new(8443)?;
    // 唯一属主。
    let entries = vec![open_port(Some(7), 8443), open_port(Some(7), 9000)];
    assert_eq!(
        resolve_port_owner(&entries, port)?,
        Resolution::Unique(Pid::new(7)?)
    );
    // 多属主 → Ambiguous，完整且升序。
    let entries = vec![open_port(Some(9), 8443), open_port(Some(3), 8443)];
    let Resolution::Ambiguous(owners) = resolve_port_owner(&entries, port)? else {
        return Err(String::from("多属主应返回 Ambiguous").into());
    };
    assert_eq!(owners, vec![Pid::new(3)?, Pid::new(9)?]);
    // 有条目但属主不可知 → SocketOwnerUnknown（parity 哨兵）。
    let entries = vec![open_port(None, 8443)];
    let err = resolve_port_owner(&entries, port)
        .err()
        .ok_or_else(|| String::from("无主端口应报 socket_owner_unknown"))?;
    assert_eq!(err.code(), "socket_owner_unknown");
    // 无条目 → NotFound。
    let entries = vec![open_port(Some(7), 9000)];
    let err = resolve_port_owner(&entries, port)
        .err()
        .ok_or_else(|| String::from("无条目应报 not_found"))?;
    assert_eq!(err.code(), "not_found");
    Ok(())
}

/// `SocketEntry` 上的端口解析与 `OpenPortEntry` 行为一致。
#[test]
fn target_port_resolution_on_socket_entries_matches_open_port_semantics() -> TestResult {
    let socket = |owner: Option<u32>, port: u16| SocketEntry {
        inode: None,
        port: Port::new(port).ok(),
        address: String::from("0.0.0.0"),
        remote_addr: None,
        state: String::from("LISTEN"),
        protocol: Protocol::Tcp,
        owner_pid: owner.and_then(|p| Pid::new(p).ok()),
    };
    let port = Port::new(80)?;
    let sockets = vec![socket(Some(2), 80), socket(Some(2), 443)];
    assert_eq!(
        resolve_port_owner_in_sockets(&sockets, port)?,
        Resolution::Unique(Pid::new(2)?)
    );
    let sockets = vec![socket(None, 80)];
    assert_eq!(
        resolve_port_owner_in_sockets(&sockets, port)
            .err()
            .map(|e| e.code().to_string()),
        Some(String::from("socket_owner_unknown"))
    );
    Ok(())
}
