use runquiry_core::{ContainerInventory, ContainerKey, ContainerProcessVerifier, Pid};
use runquiry_platform::container::{ContainerRuntimes, RuntimeBinaries};

use crate::support::{TempDir, TestResult, absent_binaries, fake_cli, subcommand_response};

struct Membership(bool);

impl ContainerProcessVerifier for Membership {
    fn belongs_to_container(&self, _pid: Pid, _key: &ContainerKey) -> bool {
        self.0
    }
}

#[test]
fn container_host_pid_unknown_runtime_is_unsupported() -> TestResult {
    let dir = TempDir::new("unknown-runtime")?;
    let docker = fake_cli(&dir, "docker", "")?;
    let inventory = ContainerRuntimes::with_binaries(RuntimeBinaries {
        docker: docker.display().to_string(),
        ..absent_binaries()
    });
    let key = ContainerKey {
        runtime: String::from("jail"),
        id: String::from("abc"),
    };
    assert!(matches!(
        ContainerInventory::host_pid(&inventory, &key),
        Err(runquiry_core::InspectError::Unsupported { .. })
    ));
    Ok(())
}

#[test]
fn container_host_pid_dispatches_to_owning_runtime() -> TestResult {
    let dir = TempDir::new("dispatch")?;
    let docker = fake_cli(
        &dir,
        "docker",
        &subcommand_response(
            "inspect",
            r#"{"Pid":4242,"StartedAt":"2026-08-30T10:00:00Z"}"#,
        ),
    )?;
    let inventory = ContainerRuntimes::with_binaries(RuntimeBinaries {
        docker: docker.display().to_string(),
        ..absent_binaries()
    });
    let key = ContainerKey {
        runtime: String::from("docker"),
        id: String::from("1111111111111111111111111111111111111111111111111111111111111111"),
    };
    assert_eq!(
        ContainerInventory::host_pid(&inventory, &key)?,
        Some(Pid::new(4242)?)
    );
    assert_eq!(
        inventory.verified_host_pid(&key, &Membership(true))?,
        Some(Pid::new(4242)?)
    );
    assert_eq!(inventory.verified_host_pid(&key, &Membership(false))?, None);
    Ok(())
}
