//! Win32 / NTSTATUS 错误码 → core 诊断与 `InspectError` 的纯映射层。
//!
//! 本文件只依赖 `std` + `runquiry-core`，可在任何平台编译；Linux 上经
//! `#[path = "../src/windows/winerror.rs"]` 由 `tests/windows_winerror.rs`
//! 直接执行。错误码数值与 `windows-sys 0.61.2`（`Win32::Foundation`）一致，
//! 复制为本地常量以保持纯逻辑无 OS 依赖；新增错误码时两边同步。

use runquiry_core::{DiagnosticCode, DiagnosticIssue, InspectError};

/// `ERROR_SUCCESS`（= 0；GetExtended*Table 等成功返回值）。
pub const ERROR_SUCCESS: u32 = 0;
/// `ERROR_ACCESS_DENIED`（windows-sys `Win32::Foundation` = 5）。
pub const ERROR_ACCESS_DENIED: u32 = 5;
/// `ERROR_INVALID_HANDLE`（= 6）。
pub const ERROR_INVALID_HANDLE: u32 = 6;
/// `ERROR_INVALID_PARAMETER`（= 87；OpenProcess 对已消失/受保护进程的常见返回）。
pub const ERROR_INVALID_PARAMETER: u32 = 87;
/// `ERROR_INSUFFICIENT_BUFFER`（= 122；表/枚举尺寸探测的预期信号）。
pub const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
/// `ERROR_MORE_DATA`（= 234；SCM 枚举尺寸探测与表重试的预期信号）。
pub const ERROR_MORE_DATA: u32 = 234;
/// `ERROR_NO_MORE_FILES`（= 18；Toolhelp32 遍历结束的预期信号）。
pub const ERROR_NO_MORE_FILES: u32 = 18;
/// `ERROR_SERVICE_DOES_NOT_EXIST`（= 1060；服务目标不存在）。
pub const ERROR_SERVICE_DOES_NOT_EXIST: u32 = 1060;
/// `STATUS_INFO_LENGTH_MISMATCH`（ntdll，0xC0000004；NtQuery 缓冲区不足）。
pub const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004_u32 as i32;

/// 一次 Win32 调用的原始错误码（`GetLastError`）。
///
/// 不实现 `From<io::Error>`：调用点显式记录来源，避免把 NTSTATUS 混入。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Win32Error(pub u32);

impl Win32Error {
    /// 是否为访问拒绝（映射到 [`DiagnosticCode::PermissionDenied`]）。
    #[must_use]
    pub const fn is_access_denied(self) -> bool {
        self.0 == ERROR_ACCESS_DENIED
    }

    /// 是否为「进程已消失或参数非法」类错误（详情采集回退部分结果的信号）。
    #[must_use]
    pub const fn is_target_gone_or_invalid(self) -> bool {
        matches!(self.0, ERROR_INVALID_PARAMETER | ERROR_INVALID_HANDLE)
    }
}

/// 单点系统调用失败 → 诊断（权限拒绝与未归类分别编码）。
#[must_use]
pub fn diagnostic_for(error: Win32Error, subject: &str) -> DiagnosticIssue {
    let code = if error.is_access_denied() {
        DiagnosticCode::PermissionDenied
    } else {
        DiagnosticCode::Unknown
    };
    DiagnosticIssue::new(
        code,
        format!("{subject} 的 Windows API 调用失败（Win32 错误码 {}）", error.0),
    )
}

/// 详情采集的硬身份失败映射：访问拒绝 → `PermissionDenied`，
/// 其余（进程已消失 / 参数非法）→ `NotFound`。
#[must_use]
pub fn details_error_for(error: Win32Error, subject: String) -> InspectError {
    if error.is_access_denied() {
        InspectError::PermissionDenied { subject }
    } else {
        InspectError::NotFound { subject }
    }
}

/// NTSTATUS 是否为失败码（最高位为 1 的状态为严重性 >= 警告的错误）。
#[must_use]
pub const fn is_ntstatus_failure(status: i32) -> bool {
    status < 0
}

#[cfg(test)]
mod tests {
    use super::{
        diagnostic_for, details_error_for, is_ntstatus_failure, DiagnosticCode, InspectError,
        Win32Error, ERROR_ACCESS_DENIED, ERROR_INSUFFICIENT_BUFFER, ERROR_INVALID_PARAMETER,
        ERROR_MORE_DATA, ERROR_SERVICE_DOES_NOT_EXIST, STATUS_INFO_LENGTH_MISMATCH,
    };

    #[test]
    fn error_code_values_match_windows_sys() {
        assert_eq!(ERROR_ACCESS_DENIED, 5);
        assert_eq!(ERROR_INSUFFICIENT_BUFFER, 122);
        assert_eq!(ERROR_MORE_DATA, 234);
        assert_eq!(ERROR_INVALID_PARAMETER, 87);
        assert_eq!(ERROR_SERVICE_DOES_NOT_EXIST, 1060);
        assert_eq!(STATUS_INFO_LENGTH_MISMATCH, 0xC000_0004_u32 as i32);
    }

    #[test]
    fn access_denied_maps_to_permission_denied_diagnostic() {
        let issue = diagnostic_for(Win32Error(ERROR_ACCESS_DENIED), "进程 4");
        assert_eq!(issue.code(), DiagnosticCode::PermissionDenied);
        assert!(!issue.message().contains("fxt"), "诊断不携带进程数据");
    }

    #[test]
    fn unclassified_error_maps_to_unknown_diagnostic() {
        let issue = diagnostic_for(Win32Error(9999), "TCP 表");
        assert_eq!(issue.code(), DiagnosticCode::Unknown);
    }

    #[test]
    fn details_errors_split_by_error_class() {
        assert!(matches!(
            details_error_for(Win32Error(ERROR_ACCESS_DENIED), String::from("进程 4")),
            InspectError::PermissionDenied { .. }
        ));
        assert!(matches!(
            details_error_for(Win32Error(ERROR_INVALID_PARAMETER), String::from("进程 4")),
            InspectError::NotFound { .. }
        ));
    }

    #[test]
    fn ntstatus_failure_detection() {
        assert!(is_ntstatus_failure(STATUS_INFO_LENGTH_MISMATCH));
        assert!(!is_ntstatus_failure(0));
    }
}