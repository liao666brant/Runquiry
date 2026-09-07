//! Docker 发布端口回退：固定 argv、命中、空结果与失败诊断。
#![cfg(unix)]

#[path = "container_support.rs"]
mod support;

use runquiry_core::{ContainerInventory, DiagnosticCode, Port};
use runquiry_platform::container::{ContainerRuntimes, RuntimeBinaries};
use support::{TempDir, TestResult, absent_binaries, fake_cli};

fn inventory_with_body(dir: &TempDir, body: &str) -> Result<ContainerRuntimes, std::io::Error> {
    let docker = fake_cli(dir, "docker", body)?;
    Ok(ContainerRuntimes::with_binaries(RuntimeBinaries {
        docker: docker.display().to_string(),
        ..absent_binaries()
    }))
}

#[test]
fn published_port_uses_fixed_argv_and_returns_candidate() -> TestResult {
    let dir = TempDir::new("published-port-hit")?;
    let argv = dir.path().join("argv");
    let body = format!(
        "if [ \"$1\" = \"ps\" ]; then\n  printf '%s\\n' \"$@\" > {}\n  if [ \"$2\" = \"--filter\" ] && [ \"$3\" = \"publish=8080\" ] && [ \"$4\" = \"--no-trunc\" ] && [ \"$5\" = \"--format\" ] && [ \"$6\" = \"{{{{json .}}}}\" ]; then\n    echo '{{\"ID\":\"abc123def456\",\"Names\":[\"web\"],\"Image\":\"app:1\",\"Command\":\"serve\",\"State\":\"running\",\"Status\":\"Up\"}}'\n    exit 0\n  fi\nfi\n",
        argv.display()
    );
    let inventory = inventory_with_body(&dir, &body)?;
    let result = ContainerInventory::published_on(&inventory, Port::new(8080)?);

    let items = result.data.ok_or("发布端口应命中容器")?;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name.as_deref(), Some("web"));
    assert_eq!(
        std::fs::read_to_string(argv)?,
        "ps\n--filter\npublish=8080\n--no-trunc\n--format\n{{json .}}\n"
    );
    Ok(())
}

#[test]
fn published_port_empty_output_is_successful_empty_list() -> TestResult {
    let dir = TempDir::new("published-port-empty")?;
    let inventory = inventory_with_body(&dir, "if [ \"$1\" = \"ps\" ]; then exit 0; fi\n")?;

    let result = inventory.published_on(Port::new(8080)?);
    assert!(result.data.ok_or("空结果仍应采集成功")?.is_empty());
    Ok(())
}

#[test]
fn published_port_failure_is_typed_diagnostic() -> TestResult {
    let dir = TempDir::new("published-port-fail")?;
    let inventory = inventory_with_body(&dir, "if [ \"$1\" = \"ps\" ]; then exit 23; fi\n")?;

    let result = inventory.published_on(Port::new(8080)?);
    assert_eq!(result.data, None);
    assert!(
        result
            .issues
            .iter()
            .any(|issue| issue.code() == DiagnosticCode::ExternalToolFailed)
    );
    Ok(())
}
