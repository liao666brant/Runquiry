use std::time::{Duration, SystemTime};

use runquiry_core::{CapabilityStatus, InspectError, Pid, ProcessAction, ProcessIdentity};

use super::{ActionCompletion, ProcessActionFlow, SuccessDisposition, accepts_action_shortcut};

fn identity(pid: u32, age: u64) -> ProcessIdentity {
    ProcessIdentity::new(
        Pid::new(pid).unwrap_or(Pid::MIN),
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(age)),
        None,
    )
}

#[test]
fn cancel_never_creates_an_execution_request() {
    let mut flow = ProcessActionFlow::new();
    assert!(flow.request(
        &CapabilityStatus::Supported,
        identity(7, 10),
        ProcessAction::Terminate,
    ));
    assert!(flow.cancel_confirmation());
    assert!(flow.confirm().is_none());
    assert!(!flow.is_busy());
}

#[test]
fn confirm_freezes_identity_and_rejects_duplicate_submission() {
    let expected = identity(7, 10);
    let mut flow = ProcessActionFlow::new();
    assert!(flow.request(
        &CapabilityStatus::Supported,
        expected.clone(),
        ProcessAction::Kill,
    ));
    assert!(flow.is_confirming());
    assert!(!flow.is_executing());
    let first = flow.confirm();
    assert!(first.is_some());
    let Some(first) = first else { return };
    assert!(!flow.is_confirming());
    assert!(flow.is_executing());
    assert!(first.identity().same_process(&expected));
    assert_eq!(first.action(), ProcessAction::Kill);
    assert!(flow.confirm().is_none());
    assert!(!flow.request(
        &CapabilityStatus::Supported,
        identity(8, 20),
        ProcessAction::Pause,
    ));
}

#[test]
fn stale_completion_cannot_replace_a_new_context() {
    let mut flow = ProcessActionFlow::new();
    assert!(flow.request(
        &CapabilityStatus::Supported,
        identity(7, 10),
        ProcessAction::Pause,
    ));
    let request = flow.confirm();
    assert!(request.is_some());
    let Some(request) = request else { return };
    flow.invalidate_context();
    assert!(flow.complete(&request, Ok(())).is_none());
    assert!(!flow.is_busy());
    assert!(flow.last_error().is_none());
}

#[test]
fn completion_classifies_refresh_and_explicit_errors() {
    let mut flow = ProcessActionFlow::new();
    assert!(flow.request(
        &CapabilityStatus::Supported,
        identity(7, 10),
        ProcessAction::Terminate,
    ));
    let request = flow.confirm();
    assert!(request.is_some());
    let Some(request) = request else { return };
    assert!(matches!(
        flow.complete(&request, Ok(())),
        Some(ActionCompletion::Succeeded(
            SuccessDisposition::ReturnToList
        ))
    ));

    assert!(flow.request(
        &CapabilityStatus::Supported,
        identity(7, 10),
        ProcessAction::Resume,
    ));
    let request = flow.confirm();
    assert!(request.is_some());
    let Some(request) = request else { return };
    assert!(matches!(
        flow.complete(&request, Ok(())),
        Some(ActionCompletion::Succeeded(
            SuccessDisposition::RefreshDetail
        ))
    ));

    assert!(flow.request(
        &CapabilityStatus::Supported,
        identity(7, 10),
        ProcessAction::Pause,
    ));
    let request = flow.confirm();
    assert!(request.is_some());
    let Some(request) = request else { return };
    let error = InspectError::PermissionDenied {
        subject: String::from("PID 7"),
    };
    assert!(matches!(
        flow.complete(&request, Err(error)),
        Some(ActionCompletion::Failed(
            InspectError::PermissionDenied { .. }
        ))
    ));
    assert!(matches!(
        flow.last_error(),
        Some(InspectError::PermissionDenied { .. })
    ));
}

#[test]
fn unsupported_and_unavailable_capabilities_never_enter_confirmation() {
    for capability in [
        CapabilityStatus::Unsupported(String::from("platform boundary")),
        CapabilityStatus::Unavailable(String::from("controller missing")),
    ] {
        let mut flow = ProcessActionFlow::new();
        assert!(!flow.request(&capability, identity(7, 10), ProcessAction::Kill));
        assert!(flow.confirm().is_none());
    }
}

#[test]
fn process_changed_failure_remains_typed_for_identity_specific_copy() {
    let mut flow = ProcessActionFlow::new();
    assert!(flow.request(
        &CapabilityStatus::Supported,
        identity(7, 10),
        ProcessAction::Kill,
    ));
    let request = flow.confirm();
    assert!(request.is_some());
    let Some(request) = request else { return };
    let current = identity(7, 20);

    assert!(matches!(
        flow.complete(
            &request,
            Err(InspectError::ProcessChanged { identity: current })
        ),
        Some(ActionCompletion::Failed(
            InspectError::ProcessChanged { .. }
        ))
    ));
}

#[test]
fn action_shortcuts_never_consume_text_input() {
    assert!(accepts_action_shortcut(true, false));
    assert!(!accepts_action_shortcut(true, true));
    assert!(!accepts_action_shortcut(false, false));
}

#[test]
fn confirm_gate_rejects_degraded_capability_without_background_request() {
    let mut flow = ProcessActionFlow::new();
    assert!(flow.request(
        &CapabilityStatus::Supported,
        identity(7, 10),
        ProcessAction::Terminate,
    ));
    let capability = CapabilityStatus::Unavailable(String::from("collector unavailable"));
    assert!(flow.confirm_if_usable(&capability).is_none());
    assert!(!flow.is_confirming());
    assert!(!flow.is_executing());
    assert!(matches!(
        flow.last_error(),
        Some(InspectError::Unsupported { reason }) if reason.as_str() == "collector unavailable"
    ));
}

#[test]
fn confirm_gate_with_usable_capability_produces_the_request() {
    let mut flow = ProcessActionFlow::new();
    assert!(flow.request(
        &CapabilityStatus::Partial(String::from("limited")),
        identity(7, 10),
        ProcessAction::Pause,
    ));
    let request = flow.confirm_if_usable(&CapabilityStatus::Partial(String::from("limited")));
    assert!(request.is_some());
    let Some(request) = request else { return };
    assert!(flow.is_executing());
    assert!(!flow.is_confirming());
    assert_eq!(request.action(), ProcessAction::Pause);
}

#[test]
fn presentation_result_never_voids_a_pending_confirmation() {
    let mut flow = ProcessActionFlow::new();
    assert!(flow.request(
        &CapabilityStatus::Supported,
        identity(7, 10),
        ProcessAction::Kill,
    ));

    // 展示类结果不得清空确认中的请求（否则点「确定」会静默失效）。
    flow.report_presentation_result(Err(InspectError::NotFound {
        subject: String::from("可执行文件路径"),
    }));

    assert!(flow.is_confirming());
    assert!(flow.confirm().is_some());
}

#[test]
fn presentation_result_is_dropped_while_an_action_executes() {
    let mut flow = ProcessActionFlow::new();
    assert!(flow.request(
        &CapabilityStatus::Supported,
        identity(7, 10),
        ProcessAction::Kill,
    ));
    let request = flow.confirm();
    assert!(request.is_some());

    flow.report_presentation_result(Err(InspectError::PermissionDenied {
        subject: String::from("可执行文件路径"),
    }));
    flow.report_presentation_result(Ok(()));

    // 执行中的动作请求与错误槽都不受展示类结果影响。
    assert!(flow.is_executing());
    assert!(flow.last_error().is_none());
}

#[test]
fn presentation_success_clears_a_previous_error() {
    let mut flow = ProcessActionFlow::new();
    flow.report_error(InspectError::InvalidTarget {
        reason: String::from("PID 非法"),
    });
    assert!(flow.last_error().is_some());

    flow.report_presentation_result(Ok(()));

    assert!(flow.last_error().is_none());
    assert!(!flow.is_busy());
}
