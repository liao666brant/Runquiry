//! 来源与摘要模型契约。

use runquiry_core::{ContainerContext, HealthStatus, Resolution, Source, SourceType};

use super::{LONG_HEX, TestResult};
#[test]
fn pipeline_container_context_reader_and_healthcheck_default() {
    let ctx = ContainerContext::new(String::from(LONG_HEX), String::from("docker"), None);
    assert_eq!(ctx.container_id(), LONG_HEX);
    assert_eq!(ctx.runtime(), "docker");
    assert_eq!(ctx.healthcheck(), None);
}

#[test]
fn pipeline_source_construction_readers_and_detail_ordering() {
    let source = Source::new(SourceType::Systemd)
        .with_name(String::from("nginx.service"))
        .with_description(String::from("A high performance web server"))
        .with_unit_file(String::from("/usr/lib/systemd/system/nginx.service"))
        .with_detail(String::from("NRestarts"), String::from("7"));

    assert_eq!(source.source_type(), SourceType::Systemd);
    assert_eq!(source.name(), Some("nginx.service"));
    assert_eq!(source.description(), Some("A high performance web server"));
    assert_eq!(
        source.unit_file(),
        Some("/usr/lib/systemd/system/nginx.service")
    );
    // details 为有序键值：core 只透传平台采集顺序，不重排。
    assert_eq!(
        source.details(),
        &[(String::from("NRestarts"), String::from("7"))]
    );
}

#[test]
fn pipeline_source_unknown_is_the_never_blank_fallback() {
    // parity：来源识别永不返回空（Detect 末尾兜底 SourceUnknown）。
    let fallback = Source::unknown();
    assert_eq!(fallback.source_type(), SourceType::Unknown);
    assert_eq!(fallback.name(), None);
    assert_eq!(fallback.details(), &[] as &[(String, String)]);
}

#[test]
fn pipeline_source_type_codes_match_witr_strings() {
    assert_eq!(SourceType::Container.code(), "container");
    assert_eq!(SourceType::Ssh.code(), "ssh");
    assert_eq!(SourceType::Shell.code(), "shell");
    assert_eq!(SourceType::Systemd.code(), "systemd");
    assert_eq!(SourceType::Launchd.code(), "launchd");
    assert_eq!(SourceType::BsdRc.code(), "bsdrc");
    assert_eq!(SourceType::Supervisor.code(), "supervisor");
    assert_eq!(SourceType::Cron.code(), "cron");
    assert_eq!(SourceType::WindowsService.code(), "windows_service");
    assert_eq!(SourceType::Init.code(), "init");
    assert_eq!(SourceType::Unknown.code(), "unknown");
}

#[test]
fn pipeline_resolution_keeps_all_candidates_without_auto_selection() {
    let unique: Resolution<u32> = Resolution::Unique(42);
    assert_eq!(unique.count(), 1);
    assert_eq!(unique.into_candidates(), vec![42]);

    // 多结果不自动选择：Ambiguous 携带完整、稳定排序的候选集合。
    let ambiguous: Resolution<u32> = Resolution::Ambiguous(vec![2, 7, 11]);
    assert_eq!(ambiguous.count(), 3);
    assert_eq!(ambiguous.into_candidates(), vec![2, 7, 11]);
}

#[test]
fn pipeline_additive_summary_fields_default_when_absent_in_json() -> TestResult {
    // 序列化兼容红线：缺少新增字段的旧条目必须按默认值读取。
    let legacy = r#"{
        "identity": {"pid": 4242, "start_time": null, "executable": null},
        "parent_pid": null,
        "command": "fxt-daemon",
        "command_line": null,
        "user": null
    }"#;
    let summary: runquiry_core::ProcessSummary = serde_json::from_str(legacy)?;
    assert_eq!(summary.health, HealthStatus::Unknown);
    assert_eq!(summary.container, None);
    assert!(!summary.exe_deleted);
    assert!(summary.capabilities.is_empty());
    Ok(())
}

#[test]
fn pipeline_additive_summary_fields_round_trip() -> TestResult {
    let summary = runquiry_core::ProcessSummary {
        identity: runquiry_core::ProcessIdentity::new(runquiry_core::Pid::new(7)?, None, None),
        parent_pid: None,
        command: String::from("fxt-daemon"),
        command_line: None,
        user: None,
        health: HealthStatus::HighCpu,
        container: Some(ContainerContext::new(
            String::from(LONG_HEX),
            String::from("docker"),
            Some(runquiry_core::HealthcheckStatus::Absent),
        )),
        exe_deleted: true,
        capabilities: vec![String::from("CAP_SYS_ADMIN")],
    };
    let text = serde_json::to_string(&summary)?;
    assert!(
        text.contains(r#""health":"high-cpu""#),
        "序列化标签与 witr 一致：{text}"
    );
    assert!(
        text.contains(r#""exe_deleted":true"#),
        "新增字段参与序列化：{text}"
    );
    let back: runquiry_core::ProcessSummary = serde_json::from_str(&text)?;
    assert_eq!(back.health, HealthStatus::HighCpu);
    assert!(back.exe_deleted);
    assert_eq!(back.capabilities, vec![String::from("CAP_SYS_ADMIN")]);
    assert_eq!(
        back.container
            .as_ref()
            .and_then(runquiry_core::ContainerContext::healthcheck),
        Some(runquiry_core::HealthcheckStatus::Absent)
    );
    Ok(())
}
