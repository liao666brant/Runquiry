//! 环境变量与最小上下文告警。

use std::time::SystemTime;

use runquiry_core::{ProcessDetails, SocketEntry, SourceType, WarningKind, WarningsContext};

use super::support::{kinds, target};

#[test]
fn warnings_suspicious_env_warnings_are_deterministic_and_ordered() {
    let sockets: Vec<SocketEntry> = Vec::new();
    let target = target();
    let details_with = |environment: &[(&str, &str)]| ProcessDetails {
        identity: target.identity.clone(),
        cpu_percent: None,
        memory_rss_bytes: None,
        memory_percent: None,
        working_dir: None,
        environment: environment
            .iter()
            .map(|(key, value)| (String::from(*key), String::from(*value)))
            .collect(),
        children: Vec::new(),
        memory: None,
        io: None,
        open_files: Vec::new(),
        fd_count: None,
        fd_limit: None,
    };
    let warnings_for = |details: &ProcessDetails| {
        runquiry_core::warnings(&WarningsContext {
            target: &target,
            sockets: &sockets,
            details: Some(details),
            service: None,
            restart_count: 0,
            source_type: SourceType::Shell,
            is_windows: false,
            now: SystemTime::UNIX_EPOCH,
        })
    };
    let details = details_with(&[
        ("LD_PRELOAD", "/opt/evil.so"),
        ("DYLD_INSERT_LIBRARIES", "/x"),
        ("DYLD_FRAMEWORK_PATH", "/y"),
    ]);
    let warnings = warnings_for(&details);
    assert_eq!(
        kinds(&warnings),
        vec![WarningKind::SuspiciousEnv, WarningKind::SuspiciousEnv]
    );
    assert_eq!(
        warnings[0].message(),
        "Process sets LD_PRELOAD (potential library injection)"
    );
    assert_eq!(
        warnings[1].message(),
        "Process sets DYLD_* variables (potential library injection): DYLD_FRAMEWORK_PATH, DYLD_INSERT_LIBRARIES"
    );
    let details = details_with(&[("LD_PRELOAD", ""), ("PATH", "/usr/bin")]);
    assert!(warnings_for(&details).is_empty());
}

#[test]
fn warnings_minimal_context_yields_no_warnings() {
    let target = target();
    let sockets: Vec<SocketEntry> = Vec::new();
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
