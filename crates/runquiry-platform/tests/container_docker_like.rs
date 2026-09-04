#![cfg(unix)]
//! docker-like 家族（docker / podman / nerdctl）解析与行为测试：假 CLI 伪造
//! 机器格式输出，覆盖正常/空/损坏/截断/字段类型错误、host PID、富集与
//! Compose 临时键的解析阶段边界。

#[path = "container_support.rs"]
mod support;

use runquiry_core::{ContainerKey, DiagnosticCode, Inspection, Pid};
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

fn docker_state(pid: &str, started_at: &str) -> String {
    format!(r#"{{"Pid":{pid},"StartedAt":"{started_at}"}}"#)
}

#[test]
fn container_docker_list_parses_lines_health_and_compose_keys() -> TestResult {
    let dir = TempDir::new("docker-list")?;
    let docker = fake_cli(&dir, "docker", &subcommand_response("ps", DOCKER_PS_LINES))?;
    let inventory = docker_only(RuntimeBinaries {
        docker: docker.display().to_string(),
        ..absent_binaries()
    });
    let inspection = inventory.list_detailed();
    let items = inspection.data.as_ref().ok_or("应有 docker 数据")?;
    let docker_items: Vec<_> = items
        .iter()
        .filter(|entry| entry.summary.key.runtime == "docker")
        .collect();
    assert_eq!(docker_items.len(), 2);
    let web = &docker_items[0];
    assert_eq!(
        web.summary.key.id,
        "5f2d4a1b9c8e0000000000000000000000000000000000000000000000000aa1"
    );
    assert_eq!(web.summary.name.as_deref(), Some("web"));
    assert_eq!(
        web.summary.image.as_deref(),
        Some("registry.example.internal/app:1")
    );
    assert_eq!(
        web.summary.status.as_deref(),
        Some("Up 4 minutes (healthy)")
    );
    assert_eq!(web.summary.health.as_deref(), Some("healthy"));
    assert_eq!(web.summary.host_pid, None);
    // Compose 临时键只存在于解析阶段结构。
    assert_eq!(web.compose_project.as_deref(), Some("demo"));
    assert_eq!(web.compose_service.as_deref(), Some("web"));
    // "health: starting" 归一为 starting。
    assert_eq!(docker_items[1].summary.health.as_deref(), Some("starting"));
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
    let items = inventory.list_detailed().data.ok_or("应有 podman 数据")?;
    let entry = items
        .iter()
        .find(|entry| entry.summary.key.runtime == "podman")
        .ok_or("podman 条目缺失")?;
    assert_eq!(entry.summary.key.id, "abc123def456");
    assert_eq!(entry.summary.name.as_deref(), Some("db"));
    assert_eq!(entry.compose_project.as_deref(), Some("stack"));
    Ok(())
}

#[test]
fn container_nerdctl_uses_containerd_display_but_nerdctl_key() -> TestResult {
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
    let items = inventory.list_detailed().data.ok_or("应有 nerdctl 数据")?;
    let entry = items
        .iter()
        .find(|entry| entry.summary.key.runtime == "nerdctl")
        .ok_or("runtime 键应为 nerdctl")?;
    assert_eq!(entry.summary.name.as_deref(), Some("solo"));
    assert_eq!(entry.compose_project, None);
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
    let inspection = inventory.list_detailed();
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
    let inspection = inventory.list_detailed();
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
    let inspection = inventory.list_detailed();
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
    let inspection: Inspection<Vec<_>> = inventory.list_detailed();
    assert_eq!(inspection.data, None);
    assert!(
        inspection
            .issues
            .iter()
            .any(|issue| issue.code() == DiagnosticCode::ParseFailed)
    );
    Ok(())
}

#[test]
fn container_docker_host_pid_from_inspect_state() -> TestResult {
    let dir = TempDir::new("docker-hostpid")?;
    let body = subcommand_response("inspect", &docker_state("4242", "2026-08-30T10:00:00Z"));
    let docker = fake_cli(&dir, "docker", &body)?;
    let inventory = docker_only(RuntimeBinaries {
        docker: docker.display().to_string(),
        ..absent_binaries()
    });
    let key = ContainerKey {
        runtime: String::from("docker"),
        id: String::from("abc123def456"),
    };
    assert_eq!(inventory.host_pid(&key)?, Some(Pid::new(4242)?));
    // enrich 取 State.StartedAt（列表只有创建时间，witr dockerLikeEnrich 语义）。
    assert!(inventory.enrich(&key)?.started_at.is_some());
    Ok(())
}

#[test]
fn container_docker_host_pid_zero_missing_or_negative_is_none() -> TestResult {
    let dir = TempDir::new("docker-pid-zero")?;
    let body = subcommand_response("inspect", r#"{"Pid":0,"StartedAt":"0001-01-01T00:00:00Z"}"#);
    let docker = fake_cli(&dir, "docker", &body)?;
    let inventory = docker_only(RuntimeBinaries {
        docker: docker.display().to_string(),
        ..absent_binaries()
    });
    let key = ContainerKey {
        runtime: String::from("docker"),
        id: String::from("abc123def456"),
    };
    assert_eq!(inventory.host_pid(&key)?, None);
    // docker 零值 StartedAt 不得被当作真实启动时间。
    assert_eq!(inventory.enrich(&key)?.started_at, None);
    Ok(())
}

#[test]
fn container_docker_invalid_container_id_is_rejected_before_cli() -> TestResult {
    let dir = TempDir::new("docker-invalid-id")?;
    let docker = fake_cli(&dir, "docker", "")?;
    let inventory = docker_only(RuntimeBinaries {
        docker: docker.display().to_string(),
        ..absent_binaries()
    });
    let key = ContainerKey {
        runtime: String::from("docker"),
        id: String::from("-leading-dash"),
    };
    assert!(matches!(
        inventory.host_pid(&key),
        Err(runquiry_core::InspectError::InvalidTarget { .. })
    ));
    Ok(())
}

/// 健康检查定义探针（parity `ContainerHealthcheckStatus`）：docker/podman
/// 经 `inspect --format {{json .Config.Healthcheck}}` 判定 present/absent；
/// 非零退出 → None（不触发告警）；仅 docker/podman 可判定，非法 ID 恒 None。
#[test]
fn container_healthcheck_probe_docker_podman_only() -> TestResult {
    use runquiry_core::{ContainerHealthcheckProbe, HealthcheckStatus};

    let dir = TempDir::new("healthcheck-probe")?;
    let present = subcommand_response(
        "inspect",
        r#"{"Test":["CMD-SHELL","curl -f http://localhost/health || exit 1"]}"#,
    );
    let docker = fake_cli(&dir, "docker", &present)?;
    let podman = fake_cli(&dir, "podman", &present)?;
    let inventory = docker_only(RuntimeBinaries {
        docker: docker.display().to_string(),
        podman: podman.display().to_string(),
        ..absent_binaries()
    });
    let id = "5f2d4a1b9c8e0000000000000000000000000000000000000000000000000aa1";
    assert_eq!(
        inventory.healthcheck_status(id, "docker"),
        Some(HealthcheckStatus::Present)
    );
    assert_eq!(
        inventory.healthcheck_status(id, "podman"),
        Some(HealthcheckStatus::Present)
    );

    // 未配置 HEALTHCHECK：inspect 返回 null → Absent。
    let absent_dir = TempDir::new("healthcheck-absent")?;
    let docker_absent = fake_cli(
        &absent_dir,
        "docker",
        &subcommand_response("inspect", "null"),
    )?;
    let absent_inventory = docker_only(RuntimeBinaries {
        docker: docker_absent.display().to_string(),
        ..absent_binaries()
    });
    assert_eq!(
        absent_inventory.healthcheck_status(id, "docker"),
        Some(HealthcheckStatus::Absent)
    );

    // inspect 失败（脚本对未知子命令 exit 9）→ None，与 witr 空串语义一致。
    let fail_dir = TempDir::new("healthcheck-fail")?;
    let docker_fail = fake_cli(&fail_dir, "docker", "true\n")?;
    let inventory_fail = docker_only(RuntimeBinaries {
        docker: docker_fail.display().to_string(),
        ..absent_binaries()
    });
    assert_eq!(inventory_fail.healthcheck_status(id, "docker"), None);

    // 仅 docker/podman 可判定：nerdctl / 未知 runtime / 非法 ID 恒 None。
    let nerdctl = fake_cli(&dir, "nerdctl", &present)?;
    let with_nerdctl = docker_only(RuntimeBinaries {
        docker: docker.display().to_string(),
        nerdctl: nerdctl.display().to_string(),
        ..absent_binaries()
    });
    assert_eq!(with_nerdctl.healthcheck_status(id, "nerdctl"), None);
    assert_eq!(inventory.healthcheck_status(id, "not-a-runtime"), None);
    assert_eq!(inventory.healthcheck_status("bad id", "docker"), None);
    Ok(())
}
