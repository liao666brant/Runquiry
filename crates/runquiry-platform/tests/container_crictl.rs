#![cfg(unix)]
//! crictl 运行时（runtime 名 `k8s`）测试：列表解析、inspect 双解析
//! （老版本 info 为 JSON 字符串包装）、主机 PID 为零/缺失语义。

#[path = "container_support.rs"]
mod support;

use runquiry_core::{ContainerInventory, ContainerKey, DiagnosticCode, Pid};
use runquiry_platform::container::{ContainerRuntimes, RuntimeBinaries};
use support::{TempDir, TestResult, absent_binaries, fake_cli, subcommand_response};

fn crictl_inventory(crictl: &std::path::Path) -> ContainerRuntimes {
    ContainerRuntimes::with_binaries(RuntimeBinaries {
        crictl: crictl.display().to_string(),
        ..absent_binaries()
    })
}

fn k8s_key(id: &str) -> ContainerKey {
    ContainerKey {
        runtime: String::from("k8s"),
        id: String::from(id),
    }
}

#[test]
fn container_crictl_list_parses_containers_and_state_prefix() -> TestResult {
    let dir = TempDir::new("crictl-list")?;
    let body = subcommand_response(
        "ps",
        r#"{"containers":[{"id":"e1a2b3c4d5e6f7080000000000000000000000000000000000000000000000a1","metadata":{"name":"web-pod-1"},"image":{"image":"registry.example.internal/app:1"},"state":"CONTAINER_RUNNING","createdAt":"2026-08-30T10:00:00Z"},{"id":"00000000000000000000000000000000000000000000000000000000000bbb2","metadata":{"name":"db-pod"},"image":{"image":"registry.example.internal/db:2"},"state":"CONTAINER_EXITED"}]}"#,
    );
    let crictl = fake_cli(&dir, "crictl", &body)?;
    let inventory = crictl_inventory(&crictl);
    let items = ContainerInventory::list(&inventory)
        .data
        .ok_or("应有 k8s 数据")?;
    assert_eq!(items.len(), 2);
    let first = &items[0];
    assert_eq!(first.key.runtime, "k8s");
    assert_eq!(first.name.as_deref(), Some("web-pod-1"));
    assert_eq!(
        first.image.as_deref(),
        Some("registry.example.internal/app:1")
    );
    assert_eq!(first.status.as_deref(), Some("RUNNING"));
    assert!(
        first.started_at.is_some(),
        "crictl 的 StartedAt 取列表 createdAt"
    );
    Ok(())
}

#[test]
fn container_crictl_host_pid_direct_inspect_form() -> TestResult {
    let dir = TempDir::new("crictl-direct")?;
    let body = subcommand_response(
        "inspect",
        r#"{"status":{"startedAt":"2026-08-30T10:00:00Z"},"info":{"pid":7777,"runtimeSpec":{"process":{"args":["/app"]}}}}"#,
    );
    let crictl = fake_cli(&dir, "crictl", &body)?;
    let inventory = crictl_inventory(&crictl);
    assert_eq!(
        inventory.host_pid(&k8s_key("e1a2b3c4"))?,
        Some(Pid::new(7777)?)
    );
    assert!(inventory.enrich(&k8s_key("e1a2b3c4"))?.started_at.is_some());
    Ok(())
}

#[test]
fn container_crictl_host_pid_wrapper_inspect_form() -> TestResult {
    let dir = TempDir::new("crictl-wrapper")?;
    // 老版本 crictl：info 是 JSON 编码字符串（witr crictlInspect 双解析语义）。
    let body = subcommand_response(
        "inspect",
        r#"{"status":{"startedAt":"2026-08-30T11:00:00Z"},"info":"{\"pid\":8888}"}"#,
    );
    let crictl = fake_cli(&dir, "crictl", &body)?;
    let inventory = crictl_inventory(&crictl);
    assert_eq!(
        inventory.host_pid(&k8s_key("e1a2b3c4"))?,
        Some(Pid::new(8888)?)
    );
    assert!(inventory.enrich(&k8s_key("e1a2b3c4"))?.started_at.is_some());
    Ok(())
}

#[test]
fn container_crictl_host_pid_zero_is_none() -> TestResult {
    let dir = TempDir::new("crictl-pid-zero")?;
    let body = subcommand_response("inspect", r#"{"status":{},"info":{"pid":0}}"#);
    let crictl = fake_cli(&dir, "crictl", &body)?;
    let inventory = crictl_inventory(&crictl);
    assert_eq!(inventory.host_pid(&k8s_key("e1a2b3c4"))?, None);
    Ok(())
}

#[test]
fn container_crictl_empty_list_is_success() -> TestResult {
    let dir = TempDir::new("crictl-empty")?;
    let crictl = fake_cli(
        &dir,
        "crictl",
        &subcommand_response("ps", r#"{"containers":[]}"#),
    )?;
    let inventory = crictl_inventory(&crictl);
    let items = ContainerInventory::list(&inventory)
        .data
        .ok_or("应有空数据")?;
    assert!(items.is_empty());
    Ok(())
}

#[test]
fn container_crictl_corrupt_json_is_parse_failed() -> TestResult {
    let dir = TempDir::new("crictl-corrupt")?;
    let crictl = fake_cli(
        &dir,
        "crictl",
        &subcommand_response("ps", "{\"containers\":["),
    )?;
    let inventory = crictl_inventory(&crictl);
    let inspection = ContainerInventory::list(&inventory);
    assert_eq!(inspection.data, None);
    assert!(inspection.issues.iter().any(
        |issue| issue.code() == DiagnosticCode::ParseFailed && issue.message().contains("k8s")
    ));
    Ok(())
}
