#![cfg(unix)]
//! 汇总层测试：失败隔离（CLI 缺失 / 超时 / 非零退出时其他运行时结果仍在）、
//! runtime + id 去重（跨运行时重复短 ID 不合并）、能力状态、Compose 临时键
//! 不随 [`ContainerInventory`] 快照外泄、未知运行时与超长 ID 的边界。

#[path = "container_inventory/host_pid.rs"]
mod host_pid;
#[path = "container_support.rs"]
mod support;

use runquiry_core::{CapabilityStatus, ContainerInventory, ContainerKey, DiagnosticCode};
use runquiry_platform::container::{ContainerRuntimes, RuntimeBinaries};
use support::{TempDir, TestResult, absent_binaries, fake_cli, subcommand_response};

fn docker_lines() -> String {
    subcommand_response(
        "ps",
        r#"{"ID":"1111111111111111111111111111111111111111111111111111111111111111","Names":["web"],"Image":"registry.example.internal/app:1","State":"running","Status":"Up 1 minute","CreatedAt":"2026-08-30T10:00:00Z","Labels":{}}"#,
    )
}

#[test]
fn container_one_runtime_missing_keeps_other_runtime_results() -> TestResult {
    let dir = TempDir::new("isolate-missing")?;
    let docker = fake_cli(&dir, "docker", &docker_lines())?;
    let inventory = ContainerRuntimes::with_binaries(RuntimeBinaries {
        docker: docker.display().to_string(),
        ..absent_binaries()
    });
    let inspection = ContainerInventory::list(&inventory);
    let items = inspection
        .data
        .as_ref()
        .ok_or("docker 数据不应被 podman 缺失抹掉")?;
    assert!(items.iter().any(|entry| entry.key.runtime == "docker"));
    // CLI 缺失按任务规格记录为 ExternalToolFailed 诊断。
    assert!(
        inspection
            .issues
            .iter()
            .any(|issue| issue.code() == DiagnosticCode::ExternalToolFailed
                && issue.message().contains("podman"))
    );
    Ok(())
}

#[test]
fn container_one_runtime_timeout_keeps_other_runtime_results() -> TestResult {
    let dir = TempDir::new("isolate-timeout")?;
    let docker = fake_cli(&dir, "docker", &docker_lines())?;
    let crictl = fake_cli(
        &dir,
        "crictl",
        "if [ \"$1\" = \"ps\" ]; then\n  sleep 30\n  exit 0\nfi\n",
    )?;
    let inventory = ContainerRuntimes::with_binaries(RuntimeBinaries {
        docker: docker.display().to_string(),
        crictl: crictl.display().to_string(),
        ..absent_binaries()
    });
    let inspection = ContainerInventory::list(&inventory);
    let items = inspection
        .data
        .as_ref()
        .ok_or("docker 数据不应被 crictl 超时抹掉")?;
    assert!(items.iter().any(|entry| entry.key.runtime == "docker"));
    assert!(
        inspection
            .issues
            .iter()
            .any(|issue| issue.code() == DiagnosticCode::Timeout
                && issue.message().contains("k8s"))
    );
    Ok(())
}

#[test]
fn container_one_runtime_nonzero_exit_keeps_other_runtime_results() -> TestResult {
    let dir = TempDir::new("isolate-exit")?;
    let docker = fake_cli(&dir, "docker", &docker_lines())?;
    let podman = fake_cli(
        &dir,
        "podman",
        "if [ \"$1\" = \"ps\" ]; then\n  exit 1\nfi\n",
    )?;
    let inventory = ContainerRuntimes::with_binaries(RuntimeBinaries {
        docker: docker.display().to_string(),
        podman: podman.display().to_string(),
        ..absent_binaries()
    });
    let inspection = ContainerInventory::list(&inventory);
    let items = inspection
        .data
        .as_ref()
        .ok_or("docker 数据不应被 podman 失败抹掉")?;
    assert!(items.iter().any(|entry| entry.key.runtime == "docker"));
    assert!(
        inspection
            .issues
            .iter()
            .any(|issue| issue.code() == DiagnosticCode::ExternalToolFailed
                && issue.message().contains("podman"))
    );
    Ok(())
}

#[test]
fn container_same_short_id_across_runtimes_is_two_containers() -> TestResult {
    let dir = TempDir::new("dedup")?;
    // docker 与 podman 各自报同一短 ID：是两个容器，不得跨运行时合并。
    let docker = fake_cli(
        &dir,
        "docker",
        &subcommand_response(
            "ps",
            r#"{"ID":"deadbeefdead","Names":["from-docker"],"State":"running","Status":"Up","CreatedAt":"2026-08-30T10:00:00Z"}"#,
        ),
    )?;
    let podman = fake_cli(
        &dir,
        "podman",
        &subcommand_response(
            "ps",
            r#"[{"ID":"deadbeefdead","Names":["from-podman"],"State":"running","Status":"Up","CreatedAt":"2026-08-30T10:00:00Z"}]"#,
        ),
    )?;
    let inventory = ContainerRuntimes::with_binaries(RuntimeBinaries {
        docker: docker.display().to_string(),
        podman: podman.display().to_string(),
        ..absent_binaries()
    });
    let items = ContainerInventory::list(&inventory)
        .data
        .ok_or("应有双运行时数据")?;
    assert_eq!(items.len(), 2);
    assert!(items.iter().any(|entry| entry.key.runtime == "docker"
        && entry.key.id == "deadbeefdead"
        && entry.name.as_deref() == Some("from-docker")));
    assert!(
        items
            .iter()
            .any(|entry| entry.key.runtime == "podman" && entry.key.id == "deadbeefdead")
    );
    Ok(())
}

#[test]
fn container_duplicate_id_within_runtime_is_deduped() -> TestResult {
    let dir = TempDir::new("dedup-within")?;
    let line = r#"{"ID":"1111111111111111111111111111111111111111111111111111111111111111","Names":["web"],"State":"running","Status":"Up","CreatedAt":"2026-08-30T10:00:00Z"}"#;
    let body = subcommand_response("ps", &format!("{line}\n{line}\n"));
    let docker = fake_cli(&dir, "docker", &body)?;
    let inventory = ContainerRuntimes::with_binaries(RuntimeBinaries {
        docker: docker.display().to_string(),
        ..absent_binaries()
    });
    let items = ContainerInventory::list(&inventory)
        .data
        .ok_or("应有数据")?;
    assert_eq!(items.len(), 1);
    Ok(())
}

#[test]
fn container_capability_reflects_probe_results() -> TestResult {
    let dir = TempDir::new("capability")?;
    let all_absent = ContainerRuntimes::with_binaries(absent_binaries());
    assert!(matches!(
        all_absent.capability(),
        CapabilityStatus::Unavailable(_)
    ));
    let docker = fake_cli(&dir, "docker", &docker_lines())?;
    let docker_only = ContainerRuntimes::with_binaries(RuntimeBinaries {
        docker: docker.display().to_string(),
        ..absent_binaries()
    });
    match docker_only.capability() {
        CapabilityStatus::Partial(reason) => {
            assert!(
                reason.contains("docker") && reason.contains("podman"),
                "{reason}"
            );
        }
        other => return Err(format!("单运行时应为 Partial，实际 {other:?}").into()),
    }
    Ok(())
}

#[test]
fn container_trait_list_drops_compose_temporary_keys() -> TestResult {
    let dir = TempDir::new("compose-boundary")?;
    let docker = fake_cli(
        &dir,
        "docker",
        &subcommand_response(
            "ps",
            r#"{"ID":"1111111111111111111111111111111111111111111111111111111111111111","Names":["web"],"Image":"registry.example.internal/app:1","State":"running","Status":"Up","CreatedAt":"2026-08-30T10:00:00Z","Labels":{"com.docker.compose.project":"demo","com.docker.compose.service":"web"}}"#,
        ),
    )?;
    let inventory = ContainerRuntimes::with_binaries(RuntimeBinaries {
        docker: docker.display().to_string(),
        ..absent_binaries()
    });
    assert!(inventory.resolve("demo", true)?.data.is_some());
    assert!(inventory.resolve("web", true)?.data.is_some());
    // trait 快照只含 ContainerSummary：Compose 键不外泄（结构上无此字段）。
    let summaries = ContainerInventory::list(&inventory)
        .data
        .ok_or("应有数据")?;
    assert_eq!(summaries.len(), 1);
    assert_eq!(
        summaries[0].key,
        ContainerKey {
            runtime: String::from("docker"),
            id: String::from("1111111111111111111111111111111111111111111111111111111111111111"),
        }
    );
    Ok(())
}
