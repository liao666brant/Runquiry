//! 告警 append 顺序的对照测试。

use std::time::{Duration, SystemTime};

use runquiry_core::{
    HealthStatus, HealthcheckStatus, ProcessDetails, SourceType, WarningKind, WarningsContext,
};

use super::support::{kinds, listen_socket, target};

#[test]
fn warnings_warning_order_matches_witr_append_sequence() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_hours(91 * 24);
    let mut target = target();
    target.health = HealthStatus::Zombie;
    target.user = Some(String::from("root"));
    target.exe_deleted = true;
    target.capabilities = vec![String::from("CAP_SYS_ADMIN")];
    target.container = Some(runquiry_core::ContainerContext::new(
        String::from("fxt-id"),
        String::from("docker"),
        Some(HealthcheckStatus::Absent),
    ));
    let sockets = vec![listen_socket("0.0.0.0", "LISTEN")];
    let details = ProcessDetails {
        identity: target.identity.clone(),
        cpu_percent: None,
        memory_rss_bytes: None,
        memory_percent: None,
        working_dir: Some(std::path::PathBuf::from("/tmp")),
        environment: Vec::new(),
        children: Vec::new(),
        memory: None,
        io: None,
        open_files: Vec::new(),
        fd_count: None,
        fd_limit: None,
    };
    let context = WarningsContext {
        target: &target,
        sockets: &sockets,
        details: Some(&details),
        service: Some(String::from("totally-unrelated.service")),
        restart_count: 6,
        source_type: SourceType::Unknown,
        is_windows: false,
        now,
    };
    let warnings = runquiry_core::warnings(&context);
    assert_eq!(
        kinds(&warnings),
        vec![
            WarningKind::ServiceRestart,
            WarningKind::Zombie,
            WarningKind::PublicListen,
            WarningKind::RootUser,
            WarningKind::UnknownSource,
            WarningKind::LongRunning,
            WarningKind::SuspiciousWorkingDir,
            WarningKind::ContainerNoHealthcheck,
            WarningKind::ServiceNameMismatch,
            WarningKind::DeletedBinary,
        ],
        "顺序须与 witr 逐条一致"
    );
    assert_eq!(warnings[0].message(), "Service has restarted 6 times");
    assert_eq!(warnings[1].message(), "Process is a zombie (defunct)");
    assert_eq!(
        warnings[2].message(),
        "Process is listening on a public interface"
    );
    assert_eq!(warnings[3].message(), "Process is running as root");
    assert_eq!(
        warnings[4].message(),
        "No known supervisor or service manager detected"
    );
    assert_eq!(
        warnings[5].message(),
        "Process has been running for over 90 days"
    );
    assert_eq!(
        warnings[6].message(),
        "Process is running from a suspicious working directory: /tmp"
    );
    assert_eq!(
        warnings[7].message(),
        "Container has no healthcheck configured"
    );
    assert_eq!(
        warnings[8].message(),
        "Service name and process name do not match"
    );
    assert_eq!(
        warnings[9].message(),
        "Process is running from a deleted binary (potential library injection or pending update)"
    );
}
