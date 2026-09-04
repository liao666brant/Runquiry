//! 容器运行时缺失和端口采集超时 fixture。

use runquiry_core::{CapabilityStatus, ContainerSummary, OpenPortEntry};

use crate::support::fixtures::load;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn containers_tool_missing_fixture_reports_external_tool_and_capability() -> TestResult {
    let linux = load::<Vec<ContainerSummary>>("linux/containers-tool-missing.json")?;
    let entries = linux
        .inspection
        .data
        .as_deref()
        .ok_or_else(|| String::from("linux 容器场景必须保留 docker 数据"))?;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key.runtime, "docker");
    assert_eq!(
        linux.inspection.issues[0].code().code(),
        "external_tool_failed"
    );
    assert!(matches!(
        linux.capability,
        Some(CapabilityStatus::Partial(_))
    ));
    for platform in ["macos", "windows"] {
        let fixture =
            load::<Vec<ContainerSummary>>(&format!("{platform}/containers-tool-missing.json"))?;
        assert!(fixture.inspection.is_empty());
        assert_eq!(
            fixture.inspection.issues[0].code().code(),
            "external_tool_failed"
        );
        assert_eq!(
            fixture.capability,
            Some(CapabilityStatus::Unavailable(String::from(
                "容器运行时 CLI 未安装（合成场景）"
            )))
        );
    }
    Ok(())
}

#[test]
fn open_ports_timeout_fixture_reports_timeout() -> TestResult {
    let macos = load::<Vec<OpenPortEntry>>("macos/open-ports-timeout.json")?;
    let ports = macos
        .inspection
        .data
        .as_deref()
        .ok_or_else(|| String::from("macos 超时场景保留已取得端口"))?;
    assert_eq!(ports.len(), 1);
    assert_eq!(ports[0].port.get(), 8443);
    assert_eq!(macos.inspection.issues[0].code().code(), "timeout");
    for platform in ["linux", "windows"] {
        let fixture = load::<Vec<OpenPortEntry>>(&format!("{platform}/open-ports-timeout.json"))?;
        assert!(fixture.inspection.is_empty());
        assert_eq!(fixture.inspection.issues[0].code().code(), "timeout");
    }
    Ok(())
}
