//! 可持久化公共类型的往返契约。

use std::time::SystemTime;

use runquiry_core::{ContainerKey, Pid, Port, ProcessAction, ProcessIdentity, QueryTarget, Renice};

type TestResult = Result<(), Box<dyn std::error::Error>>;

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
    assert!(back.same_process(&identity));
    Ok(())
}
