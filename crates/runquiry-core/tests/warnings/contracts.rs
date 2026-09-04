//! 已实现告警相关领域类型的稳定契约。

use runquiry_core::{HealthStatus, InspectError, SourceType};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn warnings_health_status_labels_match_witr_strings() {
    assert_eq!(HealthStatus::Healthy.as_str(), "healthy");
    assert_eq!(HealthStatus::Zombie.as_str(), "zombie");
    assert_eq!(HealthStatus::Stopped.as_str(), "stopped");
    assert_eq!(HealthStatus::HighCpu.as_str(), "high-cpu");
    assert_eq!(HealthStatus::HighMem.as_str(), "high-mem");
    assert_eq!(HealthStatus::Unknown.as_str(), "unknown");
}

#[test]
fn warnings_health_status_serializes_to_witr_labels() -> TestResult {
    for (status, label) in [
        (HealthStatus::Healthy, "healthy"),
        (HealthStatus::Zombie, "zombie"),
        (HealthStatus::Stopped, "stopped"),
        (HealthStatus::HighCpu, "high-cpu"),
        (HealthStatus::HighMem, "high-mem"),
        (HealthStatus::Unknown, "unknown"),
    ] {
        let text = serde_json::to_string(&status)?;
        assert_eq!(
            text,
            format!(r#""{label}""#),
            "序列化值必须与 witr 标签一致"
        );
        let back: HealthStatus = serde_json::from_str(&text)?;
        assert_eq!(back, status);
    }
    Ok(())
}

#[test]
fn warnings_health_status_defaults_to_unknown() {
    assert_eq!(HealthStatus::default(), HealthStatus::Unknown);
    assert_eq!(HealthStatus::default().as_str(), "unknown");
}

#[test]
fn warnings_source_type_code_is_stable_for_warning_branches() {
    assert_eq!(SourceType::Unknown.code(), "unknown");
    assert_eq!(SourceType::Systemd.code(), "systemd");
}

#[test]
fn warnings_socket_owner_unknown_carries_stable_code() {
    let err = InspectError::SocketOwnerUnknown {
        subject: String::from("端口 8443"),
    };
    assert_eq!(err.code(), "socket_owner_unknown");
    let display = err.to_string();
    assert!(
        display.contains("端口 8443"),
        "Display 须携带主题供 UI 呈现：{display}"
    );
}
