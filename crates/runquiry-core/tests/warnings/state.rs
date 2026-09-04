//! 健康、网络、权限与来源状态告警。

use std::time::SystemTime;

use runquiry_core::{HealthStatus, SocketEntry, SourceType, WarningKind, WarningsContext};

use super::support::{kinds, listen_socket, target};

fn warnings_with_health(health: HealthStatus) -> Vec<WarningKind> {
    let sockets: Vec<SocketEntry> = Vec::new();
    let target = runquiry_core::ProcessSummary { health, ..target() };
    let context = WarningsContext {
        target: &target,
        sockets: &sockets,
        details: None,
        service: None,
        restart_count: 0,
        source_type: SourceType::Shell,
        is_windows: false,
        now: SystemTime::UNIX_EPOCH,
    };
    kinds(&runquiry_core::warnings(&context))
}

#[test]
fn warnings_health_state_warnings_follow_witr_labels() {
    assert_eq!(
        warnings_with_health(HealthStatus::Stopped),
        vec![WarningKind::Stopped]
    );
    assert_eq!(
        warnings_with_health(HealthStatus::HighCpu),
        vec![WarningKind::HighCpu]
    );
    assert_eq!(
        warnings_with_health(HealthStatus::HighMem),
        vec![WarningKind::HighMem]
    );
    assert!(
        warnings_with_health(HealthStatus::Healthy).is_empty()
            && warnings_with_health(HealthStatus::Unknown).is_empty()
    );
}

#[test]
fn warnings_public_listen_warning_requires_listening_any_address() {
    let target = target();
    for (address, state, expected) in [
        ("127.0.0.1", "LISTEN", Vec::new()),
        ("::", "LISTEN", vec![WarningKind::PublicListen]),
        ("0.0.0.0", "ESTAB", Vec::new()),
    ] {
        let sockets = vec![listen_socket(address, state)];
        let context = WarningsContext {
            target: &target,
            sockets: &sockets,
            details: None,
            service: None,
            restart_count: 0,
            source_type: SourceType::Shell,
            is_windows: false,
            now: SystemTime::UNIX_EPOCH,
        };
        assert_eq!(
            kinds(&runquiry_core::warnings(&context)),
            expected,
            "{address} {state}"
        );
    }
}

#[test]
fn warnings_root_and_dangerous_capabilities_are_mutually_exclusive() {
    let sockets: Vec<SocketEntry> = Vec::new();
    let mut target = target();
    target.user = Some(String::from("root"));
    target.capabilities = vec![String::from("CAP_SYS_ADMIN"), String::from("CAP_NET_RAW")];
    let context = WarningsContext {
        target: &target,
        sockets: &sockets,
        details: None,
        service: None,
        restart_count: 0,
        source_type: SourceType::Shell,
        is_windows: false,
        now: SystemTime::UNIX_EPOCH,
    };
    assert_eq!(
        kinds(&runquiry_core::warnings(&context)),
        vec![WarningKind::RootUser]
    );
    target.user = Some(String::from("fixture-user"));
    let context = WarningsContext {
        target: &target,
        sockets: &sockets,
        details: None,
        service: None,
        restart_count: 0,
        source_type: SourceType::Shell,
        is_windows: false,
        now: SystemTime::UNIX_EPOCH,
    };
    let warnings = runquiry_core::warnings(&context);
    assert_eq!(kinds(&warnings), vec![WarningKind::DangerousCapabilities]);
    assert!(warnings[0].message().contains("CAP_SYS_ADMIN, CAP_NET_RAW"));
    target.capabilities = vec![String::from("CAP_CHOWN")];
    let context = WarningsContext {
        target: &target,
        sockets: &sockets,
        details: None,
        service: None,
        restart_count: 0,
        source_type: SourceType::Shell,
        is_windows: false,
        now: SystemTime::UNIX_EPOCH,
    };
    assert!(runquiry_core::warnings(&context).is_empty());
}

#[test]
fn warnings_unknown_source_warning_respects_windows_exemption() {
    let sockets: Vec<SocketEntry> = Vec::new();
    let target = target();
    let mut context = WarningsContext {
        target: &target,
        sockets: &sockets,
        details: None,
        service: None,
        restart_count: 0,
        source_type: SourceType::Unknown,
        is_windows: false,
        now: SystemTime::UNIX_EPOCH,
    };
    assert_eq!(
        kinds(&runquiry_core::warnings(&context)),
        vec![WarningKind::UnknownSource]
    );
    context.is_windows = true;
    assert!(runquiry_core::warnings(&context).is_empty());
}
