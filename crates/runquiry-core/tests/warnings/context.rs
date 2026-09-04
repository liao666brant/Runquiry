//! 与进程寿命、工作目录和服务名称有关的告警。

use std::time::{Duration, SystemTime};

use runquiry_core::{
    Pid, ProcessDetails, ProcessIdentity, SocketEntry, SourceType, WarningKind, WarningsContext,
};

use super::support::{kinds, target};

fn long_running_context<'a>(
    target: &'a runquiry_core::ProcessSummary,
    sockets: &'a [SocketEntry],
    started: SystemTime,
    age: Duration,
) -> WarningsContext<'a> {
    WarningsContext {
        target,
        sockets,
        details: None,
        service: None,
        restart_count: 0,
        source_type: SourceType::Shell,
        is_windows: false,
        now: started + age,
    }
}

#[test]
fn warnings_long_running_warning_uses_injected_clock_boundary() {
    let sockets: Vec<SocketEntry> = Vec::new();
    let day = Duration::from_hours(24);
    let started = SystemTime::UNIX_EPOCH + Duration::from_hours(1);
    let baseline = target();
    for (age, expected) in [(89 * day, false), (90 * day, false), (91 * day, true)] {
        assert_eq!(
            kinds(&runquiry_core::warnings(&long_running_context(
                &baseline, &sockets, started, age
            )))
            .contains(&WarningKind::LongRunning),
            expected,
            "{age:?}"
        );
    }
    let target = runquiry_core::ProcessSummary {
        identity: ProcessIdentity::new(Pid::new(5).unwrap_or(Pid::MIN), None, None),
        ..target()
    };
    assert!(
        runquiry_core::warnings(&long_running_context(&target, &sockets, started, 200 * day))
            .is_empty()
    );
}

#[test]
fn warnings_suspicious_working_directory_list_matches_witr() {
    let sockets: Vec<SocketEntry> = Vec::new();
    let target = target();
    for (directory, expected) in [
        ("/", true),
        ("/tmp", true),
        ("/var/tmp", true),
        ("/home/fixture-user", false),
    ] {
        let details = ProcessDetails {
            identity: target.identity.clone(),
            cpu_percent: None,
            memory_rss_bytes: None,
            memory_percent: None,
            working_dir: Some(std::path::PathBuf::from(directory)),
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
            service: None,
            restart_count: 0,
            source_type: SourceType::Shell,
            is_windows: false,
            now: SystemTime::UNIX_EPOCH,
        };
        let warnings = runquiry_core::warnings(&context);
        assert_eq!(
            kinds(&warnings).contains(&WarningKind::SuspiciousWorkingDir),
            expected,
            "{directory}"
        );
        if expected {
            assert!(warnings[0].message().contains(directory));
        }
    }
}

#[test]
fn warnings_service_name_mismatch_follows_witr_svc_core_semantics() {
    let sockets: Vec<SocketEntry> = Vec::new();
    let target = target();
    for (service, expected) in [
        ("fxt-daemon.service", Vec::new()),
        ("nginx.service", vec![WarningKind::ServiceNameMismatch]),
    ] {
        let context = WarningsContext {
            target: &target,
            sockets: &sockets,
            details: None,
            service: Some(String::from(service)),
            restart_count: 0,
            source_type: SourceType::Systemd,
            is_windows: false,
            now: SystemTime::UNIX_EPOCH,
        };
        assert_eq!(
            kinds(&runquiry_core::warnings(&context)),
            expected,
            "{service}"
        );
    }
}

#[test]
fn warnings_service_name_mismatch_uses_template_base_name() {
    let sockets: Vec<SocketEntry> = Vec::new();
    let target = runquiry_core::ProcessSummary {
        command: String::from("agetty"),
        ..target()
    };
    let context = WarningsContext {
        target: &target,
        sockets: &sockets,
        details: None,
        service: Some(String::from("getty@tty1.service")),
        restart_count: 0,
        source_type: SourceType::Systemd,
        is_windows: false,
        now: SystemTime::UNIX_EPOCH,
    };
    assert!(runquiry_core::warnings(&context).is_empty());
}
