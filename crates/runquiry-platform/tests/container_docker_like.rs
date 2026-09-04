#![cfg(unix)]
//! docker-like 家族（docker / podman / nerdctl）解析与行为测试：假 CLI 伪造
//! 机器格式输出，覆盖正常/空/损坏/截断/字段类型错误、host PID、富集与
//! Compose 临时键的解析阶段边界。

#[path = "container_docker_like/details.rs"]
mod details;
#[path = "container_docker_like/healthcheck.rs"]
mod healthcheck;
#[path = "container_support.rs"]
mod support;

use runquiry_core::{ContainerInventory, DiagnosticCode, Inspection, Resolution};
use runquiry_platform::container::{ContainerRuntimes, RuntimeBinaries};
use support::{TempDir, TestResult, absent_binaries, fake_cli, subcommand_response};

const fn docker_only(bins: RuntimeBinaries) -> ContainerRuntimes {
    ContainerRuntimes::with_binaries(bins)
}

/// docker `ps --no-trunc --format {{json .}}` 的两行逐行 JSON（含健康与 Compose 标签）。
const DOCKER_PS_LINES: &str = concat!(
    r#"{"ID":"5f2d4a1b9c8e0000000000000000000000000000000000000000000000000aa1","Names":["web"],"Image":"registry.example.internal/app:1","Command":"nginx -g daemon off;","State":"running","Status":"Up 4 minutes (healthy)","CreatedAt":"2026-08-30 10:00:00 +0000 UTC","Labels":{"com.docker.compose.project":"demo","com.docker.compose.service":"web"}}"#,
    "\n",
    r#"{"ID":"00000000000000000000000000000000000000000000000000000000000bbb2","Names":["db"],"Image":"registry.example.internal/db:2","Command":"postgres","State":"running","Status":"Up 2 hours (health: starting)","CreatedAt":"2026-08-30T09:00:00Z","Labels":{}}"#,
);

#[test]
fn container_docker_list_parses_lines_health_and_compose_keys() -> TestResult {
    let dir = TempDir::new("docker-list")?;
    let docker = fake_cli(&dir, "docker", &subcommand_response("ps", DOCKER_PS_LINES))?;
    let inventory = docker_only(RuntimeBinaries {
        docker: docker.display().to_string(),
        ..absent_binaries()
    });
    let inspection = ContainerInventory::list(&inventory);
    let items = inspection.data.as_ref().ok_or("应有 docker 数据")?;
    let docker_items: Vec<_> = items
        .iter()
        .filter(|entry| entry.key.runtime == "docker")
        .collect();
    assert_eq!(docker_items.len(), 2);
    let web = &docker_items[0];
    assert_eq!(
        web.key.id,
        "5f2d4a1b9c8e0000000000000000000000000000000000000000000000000aa1"
    );
    assert_eq!(web.name.as_deref(), Some("web"));
    assert_eq!(
        web.image.as_deref(),
        Some("registry.example.internal/app:1")
    );
    assert_eq!(web.status.as_deref(), Some("Up 4 minutes (healthy)"));
    assert_eq!(web.health.as_deref(), Some("healthy"));
    assert_eq!(web.host_pid, None);
    assert!(matches!(
        inventory.resolve("demo", true)?.data,
        Some(Resolution::Unique(_))
    ));
    assert!(matches!(
        inventory.resolve("nginx -g daemon off;", true)?.data,
        Some(Resolution::Unique(_))
    ));
    // "health: starting" 归一为 starting。
    assert_eq!(docker_items[1].health.as_deref(), Some("starting"));
    Ok(())
}

#[test]
fn container_podman_list_parses_array_with_command_array_and_label_object() -> TestResult {
    let dir = TempDir::new("podman-list")?;
    let body = subcommand_response(
        "ps",
        r#"[{"ID":"abc123def456","Names":["db"],"Image":"registry.example.internal/db:2","Command":["postgres","-p","5432"],"State":"running","Status":"Up 30 minutes","CreatedAt":"2026-08-30T09:00:00Z","Labels":{"com.docker.compose.project":"stack"}}]"#,
    );
    let podman = fake_cli(&dir, "podman", &body)?;
    let inventory = docker_only(RuntimeBinaries {
        podman: podman.display().to_string(),
        ..absent_binaries()
    });
    let items = ContainerInventory::list(&inventory)
        .data
        .ok_or("应有 podman 数据")?;
    let entry = items
        .iter()
        .find(|entry| entry.key.runtime == "podman")
        .ok_or("podman 条目缺失")?;
    assert_eq!(entry.key.id, "abc123def456");
    assert_eq!(entry.name.as_deref(), Some("db"));
    assert!(matches!(
        inventory.resolve("postgres -p 5432", true)?.data,
        Some(Resolution::Unique(_))
    ));
    assert!(matches!(
        inventory.resolve("stack", true)?.data,
        Some(Resolution::Unique(_))
    ));
    Ok(())
}

#[test]
fn container_nerdctl_uses_containerd_display_and_key() -> TestResult {
    let dir = TempDir::new("nerdctl-list")?;
    let body = subcommand_response(
        "ps",
        r#"[{"ID":"c0ffee000000","Names":"solo","Image":"registry.example.internal/app:3","Command":"/app","State":"running","Status":"Up 1 minute","CreatedAt":"2026-08-30T10:00:00Z","Labels":null}]"#,
    );
    let nerdctl = fake_cli(&dir, "nerdctl", &body)?;
    let inventory = docker_only(RuntimeBinaries {
        nerdctl: nerdctl.display().to_string(),
        ..absent_binaries()
    });
    let capability = inventory.capability();
    let reason = capability
        .reason()
        .ok_or("containerd 之外无运行时，应为 Partial")?;
    assert!(
        reason.contains("containerd"),
        "显示名应为 containerd：{reason}"
    );
    let items = ContainerInventory::list(&inventory)
        .data
        .ok_or("应有 nerdctl 数据")?;
    let entry = items
        .iter()
        .find(|entry| entry.key.runtime == "containerd")
        .ok_or("runtime 键应为 containerd")?;
    assert_eq!(entry.name.as_deref(), Some("solo"));
    Ok(())
}

#[test]
fn container_docker_empty_list_is_success_without_docker_issue() -> TestResult {
    let dir = TempDir::new("docker-empty")?;
    let docker = fake_cli(&dir, "docker", &subcommand_response("ps", ""))?;
    let inventory = docker_only(RuntimeBinaries {
        docker: docker.display().to_string(),
        ..absent_binaries()
    });
    let inspection = ContainerInventory::list(&inventory);
    assert!(
        inspection
            .data
            .as_ref()
            .ok_or("部分成功应有数据")?
            .is_empty()
    );
    // docker 成功；缺失的其余运行时会产生诊断，但不得提及 docker。
    assert!(
        !inspection
            .issues
            .iter()
            .any(|issue| issue.message().contains("docker"))
    );
    Ok(())
}

#[test]
fn container_docker_corrupt_json_is_parse_failed() -> TestResult {
    let dir = TempDir::new("docker-corrupt")?;
    let docker = fake_cli(&dir, "docker", &subcommand_response("ps", "{{{not-json"))?;
    let inventory = docker_only(RuntimeBinaries {
        docker: docker.display().to_string(),
        ..absent_binaries()
    });
    let inspection = ContainerInventory::list(&inventory);
    assert_eq!(inspection.data, None);
    assert!(
        inspection
            .issues
            .iter()
            .any(|issue| issue.code() == DiagnosticCode::ParseFailed
                && issue.message().contains("docker"))
    );
    Ok(())
}

#[test]
fn container_docker_truncated_stdout_is_output_limit_exceeded() -> TestResult {
    let dir = TempDir::new("docker-truncated")?;
    // 输出 >8MiB 的截断 JSON：消费者（解析层）必须拒绝而非静默截断。
    let body = String::from(
        "if [ \"$1\" = \"ps\" ]; then\n  yes '{\"ID\":\"x\"' | head -c 9437184\n  exit 0\nfi\n",
    );
    let docker = fake_cli(&dir, "docker", &body)?;
    let inventory = docker_only(RuntimeBinaries {
        docker: docker.display().to_string(),
        ..absent_binaries()
    });
    let inspection = ContainerInventory::list(&inventory);
    assert_eq!(inspection.data, None);
    assert!(
        inspection
            .issues
            .iter()
            .any(|issue| issue.code() == DiagnosticCode::OutputLimitExceeded),
        "实际诊断: {:?}",
        inspection.issues
    );
    Ok(())
}

#[test]
fn container_docker_field_type_error_is_parse_failed() -> TestResult {
    let dir = TempDir::new("docker-type-error")?;
    let body = subcommand_response("ps", r#"{"ID":123,"Names":["web"]}"#);
    let docker = fake_cli(&dir, "docker", &body)?;
    let inventory = docker_only(RuntimeBinaries {
        docker: docker.display().to_string(),
        ..absent_binaries()
    });
    let inspection: Inspection<Vec<_>> = ContainerInventory::list(&inventory);
    assert_eq!(inspection.data, None);
    assert!(
        inspection
            .issues
            .iter()
            .any(|issue| issue.code() == DiagnosticCode::ParseFailed)
    );
    Ok(())
}
