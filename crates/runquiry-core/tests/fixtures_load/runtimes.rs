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
    let windows = load::<Vec<ContainerSummary>>("windows/containers-tool-missing.json")?;
    assert!(windows.inspection.is_empty());
    assert_eq!(
        windows.inspection.issues[0].code().code(),
        "external_tool_failed"
    );
    assert_eq!(
        windows.capability,
        Some(CapabilityStatus::Unavailable(String::from(
            "容器运行时 CLI 未安装（合成场景）"
        )))
    );
    Ok(())
}

#[test]
fn open_ports_timeout_fixture_reports_timeout() -> TestResult {
    for platform in ["linux", "windows"] {
        let fixture = load::<Vec<OpenPortEntry>>(&format!("{platform}/open-ports-timeout.json"))?;
        assert!(fixture.inspection.is_empty());
        assert_eq!(fixture.inspection.issues[0].code().code(), "timeout");
    }
    Ok(())
}
