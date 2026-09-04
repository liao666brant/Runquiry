use runquiry_core::{ContainerInventory, ContainerKey, Pid};

use super::{RuntimeBinaries, absent_binaries, docker_only, fake_cli, support::TempDir};
use crate::support::{TestResult, subcommand_response};

fn docker_state(pid: &str, started_at: &str) -> String {
    format!(r#"{{"Pid":{pid},"StartedAt":"{started_at}"}}"#)
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
