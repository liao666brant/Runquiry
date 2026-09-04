//! `SocketEntry` 的协议与 serde 边界。

use runquiry_core::{Pid, Port, Protocol, SocketEntry};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn socket_entry_covers_unix_protocol_and_remote_addr() -> TestResult {
    let port = Port::new(8080)?;
    let pid = Pid::new(31)?;
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
    assert!(text.contains("remote_addr"));
    let back: SocketEntry = serde_json::from_str(&text)?;
    assert_eq!(back.remote_addr.as_deref(), Some("10.0.0.8"));
    assert_eq!(back.protocol, Protocol::Tcp);
    assert_eq!(back.port, Some(port));
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
    let legacy: SocketEntry = serde_json::from_str(
        r#"{"inode":null,"port":8080,"address":"0.0.0.0","state":"LISTEN","protocol":"Tcp","owner_pid":null}"#,
    )?;
    assert_eq!(legacy.remote_addr, None);
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
    assert!(text.contains("\"Unix\""));
    let back: SocketEntry = serde_json::from_str(&text)?;
    assert_eq!(back.protocol, Protocol::Unix);
    assert_eq!(back.address, "/run/docker.sock");
    assert_eq!(back.port, None);
    let no_port: SocketEntry = serde_json::from_str(
        r#"{"inode":null,"address":"/run/docker.sock","state":"LISTEN","protocol":"Unix","owner_pid":null}"#,
    )?;
    assert_eq!(no_port.port, None);
    Ok(())
}
