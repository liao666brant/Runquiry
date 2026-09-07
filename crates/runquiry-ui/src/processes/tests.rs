use std::sync::Arc;
use std::time::{Duration, SystemTime};

use runquiry_core::{
    Analysis, CapabilityStatus, DiagnosticCode, DiagnosticIssue, HealthStatus, Inspection, Pid,
    ProcessDetails, ProcessIdentity, ProcessSummary, QueryTarget, Resolution, Source,
};

use super::*;

fn identity(pid: u32, age: u64) -> ProcessIdentity {
    ProcessIdentity::new(
        Pid::new(pid).unwrap_or(Pid::MIN),
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(age)),
        None,
    )
}

fn summary(pid: u32, age: u64) -> ProcessSummary {
    ProcessSummary {
        identity: identity(pid, age),
        parent_pid: None,
        command: format!("process-{pid}"),
        command_line: Some(format!("process-{pid} --token secret")),
        user: Some(String::from("tester")),
        health: HealthStatus::Healthy,
        container: None,
        exe_deleted: false,
        capabilities: Vec::new(),
    }
}

#[test]
fn explicit_target_kind_parses_all_five_queries_without_guessing() {
    let cases = [
        (TargetKind::Name, "42", "name"),
        (TargetKind::Pid, "42", "pid"),
        (TargetKind::Port, "8080", "port"),
        (TargetKind::File, "/tmp/app.sock", "file"),
        (TargetKind::Container, "web", "container"),
    ];

    for (kind, raw, expected) in cases {
        let target = kind.parse(raw, true);
        assert!(target.is_ok(), "测试输入应有效");
        let Ok(target) = target else {
            continue;
        };
        assert_eq!(target.kind(), expected);
    }
    assert!(matches!(
        TargetKind::Name.parse("42", false),
        Ok(QueryTarget::ProcessName { .. })
    ));
    assert!(TargetKind::Pid.parse("0", false).is_err());
    assert!(TargetKind::Port.parse("65536", false).is_err());
}

#[test]
fn query_outcome_never_auto_selects_an_ambiguous_candidate() {
    let first = identity(10, 10);
    let second = identity(20, 20);
    let ambiguous =
        QueryOutcome::from_resolution(Resolution::Ambiguous(vec![first.clone(), second]));

    assert_eq!(ambiguous.candidates().len(), 2);
    assert!(ambiguous.selected().is_none());
    let unique = QueryOutcome::from_resolution(Resolution::Unique(first));
    assert_eq!(unique.selected().map(|item| item.pid().get()), Some(10));
    assert!(matches!(
        QueryOutcome::<ProcessIdentity>::empty(),
        QueryOutcome::Empty
    ));
}

#[test]
fn selection_is_stable_and_pid_reuse_or_disappearance_clears_detail() {
    let rows: Arc<[ProcessSummary]> = vec![summary(7, 10), summary(8, 10)].into();
    let mut state = ProcessesState::new(rows);
    assert_eq!(state.select_row(0), SelectionChange::Selected);
    state.privacy_mut().reveal();
    let request = state.begin_detail();
    assert!(request.is_some(), "存在选择时应生成详情请求");
    let Some(request) = request else {
        return;
    };

    let reordered: Arc<[ProcessSummary]> = vec![summary(8, 10), summary(7, 10)].into();
    assert_eq!(state.replace_rows(reordered), SelectionChange::Preserved);
    assert!(!state.privacy().is_revealed());
    assert!(!state.accepts(&request));
    let request = state.begin_detail();
    assert!(request.is_some(), "刷新后的同一身份可重新请求详情");
    let Some(request) = request else {
        return;
    };

    let reused: Arc<[ProcessSummary]> = vec![summary(7, 11)].into();
    assert_eq!(state.replace_rows(reused), SelectionChange::PidReused);
    assert!(!state.privacy().is_revealed());
    assert!(!state.accepts(&request));

    state.select_row(0);
    let request = state.begin_detail();
    assert!(request.is_some(), "复用后的新身份可请求详情");
    let Some(request) = request else {
        return;
    };
    assert_eq!(
        state.replace_rows(Arc::from([])),
        SelectionChange::Disappeared
    );
    assert!(!state.accepts(&request));
}

#[test]
fn detail_generation_and_identity_both_reject_stale_results() {
    let mut state = ProcessesState::new(vec![summary(7, 10)].into());
    state.select_row(0);
    let old = state.begin_detail();
    assert!(old.is_some(), "详情请求应生成");
    let Some(old) = old else {
        return;
    };
    let current = state.begin_detail();
    assert!(current.is_some(), "新请求应推进 generation");
    let Some(current) = current else {
        return;
    };

    assert!(!state.accepts(&old));
    assert!(state.accepts(&current));
    state.clear_selection();
    assert!(!state.accepts(&current));
}

#[test]
fn cpu_none_is_sampling_and_inspection_preserves_partial_diagnostics() {
    assert_eq!(SurfaceState::cpu(None), SurfaceState::Sampling);
    assert_eq!(SurfaceState::cpu(Some(1.5)), SurfaceState::Ready);

    let issue = DiagnosticIssue::new(
        DiagnosticCode::PermissionDenied,
        String::from("环境变量不可读"),
    );
    let partial = Inspection::partial(vec![summary(7, 10)], vec![issue]);
    let surface = SurfaceSnapshot::new(&CapabilityStatus::Supported, &partial);
    assert!(matches!(
        surface.state(),
        SurfaceState::Partial { issue_count: 1 }
    ));
    assert_eq!(surface.data().map(<[_]>::len), Some(1));
    assert_eq!(surface.issues()[0].code(), DiagnosticCode::PermissionDenied);
    assert!(matches!(
        SurfaceState::from_capability(&CapabilityStatus::Unsupported(String::from("Windows"))),
        SurfaceState::Unsupported { .. }
    ));
    let unavailable = CapabilityStatus::Unavailable(String::from("collector missing"));
    assert!(matches!(
        SurfaceState::from_capability(&unavailable),
        SurfaceState::Unavailable { .. }
    ));
    let denied: Inspection<Vec<ProcessSummary>> = Inspection::failed(vec![DiagnosticIssue::new(
        DiagnosticCode::PermissionDenied,
        String::from("denied"),
    )]);
    assert_eq!(
        SurfaceSnapshot::new(&CapabilityStatus::Supported, &denied).state(),
        &SurfaceState::PermissionDenied
    );
}

#[test]
fn secrets_are_redacted_and_reveal_is_scoped_to_one_detail_session() {
    let env = vec![
        (String::from("PATH"), String::from("/bin")),
        (String::from("API_TOKEN"), String::from("top-secret")),
    ];
    let mut privacy = DetailPrivacySession::default();
    let hidden = privacy.environment(&env);
    assert_eq!(hidden[0].value(), "/bin");
    assert_eq!(hidden[1].value(), "••••••••");
    assert!(RedactedArgument::parse("--password=hunter2").is_secret());
    let hidden_args = privacy.command_line("server --token hunter2 --mode safe");
    assert_eq!(hidden_args[2].value(), "••••••••");

    privacy.reveal();
    assert_eq!(privacy.environment(&env)[1].value(), "top-secret");
    privacy.reset();
    assert_eq!(privacy.environment(&env)[1].value(), "••••••••");
}

#[test]
fn quoted_sensitive_argument_never_leaks_trailing_value_fragments() {
    let privacy = DetailPrivacySession::default();

    let hidden = privacy.command_line("server --token \"top secret\" --mode safe");
    let visible: Vec<&str> = hidden.iter().map(RedactedArgument::value).collect();

    assert_eq!(visible, vec!["server", "--token", "••••••••"]);
    assert!(
        !visible
            .iter()
            .any(|part| part.contains("top") || part.contains("secret"))
    );
}

#[test]
fn analysis_detail_model_exposes_all_sections_and_partial_issue_count() {
    let target = summary(7, 10);
    let details = ProcessDetails {
        identity: target.identity.clone(),
        cpu_percent: None,
        memory_rss_bytes: Some(42),
        memory_percent: Some(0.5),
        working_dir: None,
        environment: vec![(String::from("TOKEN"), String::from("secret"))],
        children: Vec::new(),
        memory: None,
        io: None,
        open_files: Vec::new(),
        fd_count: None,
        fd_limit: None,
    };
    let analysis = Analysis {
        target: target.clone(),
        ancestry: vec![target],
        resolved_target: String::from("process-7"),
        source: Source::unknown(),
        restart_count: 0,
        children: Vec::new(),
        sockets: Vec::new(),
        file_locks: Vec::new(),
        details: Some(details),
        warnings: Vec::new(),
    };
    let inspection = Inspection::partial(
        analysis,
        vec![DiagnosticIssue::new(
            DiagnosticCode::PermissionDenied,
            String::from("部分字段受限"),
        )],
    );

    let sections = AnalysisSections::from_inspection(&inspection);
    assert!(sections.is_some(), "部分结果仍应有详情");
    let Some(sections) = sections else {
        return;
    };
    assert_eq!(sections.sections().len(), 8);
    assert_eq!(sections.issue_count(), 1);
    assert!(
        sections
            .sections()
            .iter()
            .any(|(section, count)| *section == DetailSection::Environment && *count == 1)
    );
}

#[test]
fn large_process_model_shares_storage_and_keeps_domain_row_id_stable() {
    let rows: Arc<[ProcessSummary]> = (1..=100_000).map(|pid| summary(pid, 10)).collect();
    let model = ProcessRows::new(Arc::clone(&rows));
    let table = ProcessTableDelegate::new(Arc::clone(&rows));
    let reordered = ProcessTableDelegate::new(vec![summary(100_000, 10)].into());

    assert!(Arc::ptr_eq(&rows, model.rows()));
    assert_eq!(model.len(), 100_000);
    assert_eq!(model.visible(50_000..50_032).len(), 32);
    assert_eq!(
        table.identity_at(99_999).map(|item| item.pid().get()),
        Some(100_000)
    );
    assert_eq!(
        table.stable_row_id_at(99_999),
        reordered.stable_row_id_at(0)
    );
}

#[test]
fn keyboard_contract_maps_global_and_local_actions() {
    let bindings = ProcessCommand::bindings();
    assert!(bindings.contains(&("cmd-k", ProcessCommand::FocusQuery)));
    assert!(bindings.contains(&("ctrl-r", ProcessCommand::Refresh)));
    assert!(bindings.contains(&("ctrl-3", ProcessCommand::Workspace(3))));
}
