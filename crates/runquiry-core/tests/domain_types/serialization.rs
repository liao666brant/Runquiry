//! 可持久化公共类型的往返契约。

use std::time::SystemTime;

use runquiry_core::{
    ContainerKey, HealthStatus, Pid, Port, ProcessAction, ProcessIdentity, ProcessSummary,
    QueryTarget, Renice,
};

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

#[test]
fn process_summary_resource_fields_are_backward_compatible() -> TestResult {
    let identity = ProcessIdentity::new(Pid::new(9)?, Some(SystemTime::UNIX_EPOCH), None);
    let summary = ProcessSummary {
        identity,
        parent_pid: None,
        command: String::from("nginx"),
        command_line: None,
        user: None,
        health: HealthStatus::Unknown,
        container: None,
        exe_deleted: false,
        capabilities: Vec::new(),
        cpu_time_seconds: Some(1.5),
        cpu_percent: Some(12.0),
        memory_rss_bytes: Some(4096),
        memory_percent: Some(0.1),
    };
    // 往返保留资源字段。
    let text = serde_json::to_string(&summary)?;
    let back: ProcessSummary = serde_json::from_str(&text)?;
    assert_eq!(back.cpu_time_seconds, Some(1.5));
    assert_eq!(back.cpu_percent, Some(12.0));
    assert_eq!(back.memory_rss_bytes, Some(4096));
    assert_eq!(back.memory_percent, Some(0.1));

    // 旧序列化条目（缺资源字段）按 None 读取，反序列化不失败。
    let mut legacy = serde_json::to_value(&summary)?;
    let Some(object) = legacy.as_object_mut() else {
        return Err(String::from("序列化结果应为 JSON 对象").into());
    };
    for key in [
        "cpu_time_seconds",
        "cpu_percent",
        "memory_rss_bytes",
        "memory_percent",
    ] {
        object.remove(key);
    }
    let old: ProcessSummary = serde_json::from_value(legacy)?;
    assert_eq!(old.cpu_time_seconds, None);
    assert_eq!(old.cpu_percent, None);
    assert_eq!(old.memory_rss_bytes, None);
    assert_eq!(old.memory_percent, None);
    Ok(())
}
