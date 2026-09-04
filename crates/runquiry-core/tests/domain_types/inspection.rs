//! 能力状态、部分成功和诊断错误码契约。

use runquiry_core::{
    CapabilityStatus, DiagnosticCode, DiagnosticIssue, InspectError, Inspection, Pid,
    ProcessIdentity,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn capability_status_has_four_distinguishable_states() {
    let supported = CapabilityStatus::Supported;
    let partial = CapabilityStatus::Partial(String::from("仅支持 TCP"));
    let unsupported = CapabilityStatus::Unsupported(String::from("Windows 无文件锁"));
    let unavailable = CapabilityStatus::Unavailable(String::from("lsof 缺失"));
    assert_ne!(supported, partial);
    assert_ne!(supported, unsupported);
    assert_ne!(supported, unavailable);
    assert_ne!(partial, unsupported);
    assert_ne!(partial, unavailable);
    assert_ne!(unsupported, unavailable);
    assert!(supported.is_fully_supported());
    assert!(!partial.is_fully_supported());
    assert!(!unsupported.is_fully_supported());
    assert!(!unavailable.is_fully_supported());
    assert_eq!(partial.reason(), Some("仅支持 TCP"));
    assert_eq!(supported.reason(), None);
}

#[test]
fn inspection_keeps_data_alongside_issues() {
    let inspection: Inspection<Vec<u32>> = Inspection::partial(
        vec![1, 2, 3],
        vec![DiagnosticIssue::new(
            DiagnosticCode::PermissionDenied,
            String::from("无法读取部分条目"),
        )],
    );
    assert_eq!(inspection.data, Some(vec![1, 2, 3]));
    assert_eq!(inspection.issues.len(), 1);
    assert!(inspection.has_issues());
    assert!(!inspection.is_empty());
    let failed: Inspection<Vec<u32>> = Inspection::failed(vec![DiagnosticIssue::new(
        DiagnosticCode::Unsupported,
        String::from("平台不支持"),
    )]);
    assert_eq!(failed.data, None);
    assert!(failed.is_empty());
}

#[test]
fn inspection_serializes_with_captured_at() -> TestResult {
    let inspection = Inspection::complete(42_u32);
    let text = serde_json::to_string(&inspection)?;
    assert!(text.contains("captured_at"));
    let back: Inspection<u32> = serde_json::from_str(&text)?;
    assert_eq!(back.data, Some(42));
    Ok(())
}

#[test]
fn inspect_error_stable_codes_are_asserted_one_by_one() -> TestResult {
    let pid = Pid::new(7)?;
    let error_cases = [
        (
            InspectError::InvalidTarget {
                reason: String::from("pid=0"),
            },
            "invalid_target",
        ),
        (
            InspectError::NotFound {
                subject: String::from("nginx"),
            },
            "not_found",
        ),
        (
            InspectError::Ambiguous {
                subject: String::from("node"),
                candidate_count: 3,
            },
            "ambiguous",
        ),
        (
            InspectError::PermissionDenied {
                subject: String::from("/proc/1/environ"),
            },
            "permission_denied",
        ),
        (
            InspectError::Unsupported {
                reason: String::from("Windows 文件锁"),
            },
            "unsupported",
        ),
        (
            InspectError::ExternalTool {
                program: String::from("lsof"),
                detail: String::from("exit status 1"),
            },
            "external_tool",
        ),
        (
            InspectError::ProcessChanged {
                identity: ProcessIdentity::new(pid, None, None),
            },
            "process_changed",
        ),
    ];
    for (error, code) in error_cases {
        assert_eq!(error.code(), code, "错误码必须逐项稳定");
        assert!(!error.to_string().is_empty(), "Display 面向用户，不得为空");
    }
    Ok(())
}
