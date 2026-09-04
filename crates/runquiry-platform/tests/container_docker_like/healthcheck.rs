use runquiry_core::{ContainerHealthcheckProbe, HealthcheckStatus};

use super::{RuntimeBinaries, absent_binaries, docker_only, fake_cli, support::TempDir};
use crate::support::{TestResult, subcommand_response};

#[test]
fn container_healthcheck_probe_docker_podman_only() -> TestResult {
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

    let fail_dir = TempDir::new("healthcheck-fail")?;
    let docker_fail = fake_cli(&fail_dir, "docker", "true\n")?;
    let inventory_fail = docker_only(RuntimeBinaries {
        docker: docker_fail.display().to_string(),
        ..absent_binaries()
    });
    assert_eq!(inventory_fail.healthcheck_status(id, "docker"), None);

    let nerdctl = fake_cli(&dir, "nerdctl", &present)?;
    let with_nerdctl = docker_only(RuntimeBinaries {
        docker: docker.display().to_string(),
        nerdctl: nerdctl.display().to_string(),
        ..absent_binaries()
    });
    assert_eq!(with_nerdctl.healthcheck_status(id, "containerd"), None);
    assert_eq!(inventory.healthcheck_status(id, "not-a-runtime"), None);
    assert_eq!(inventory.healthcheck_status("bad id", "docker"), None);
    Ok(())
}
