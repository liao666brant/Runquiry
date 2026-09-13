//! Windows 不支持能力的稳定原因键与固定失败结果（纯逻辑，无 OS 依赖）。
//!
//! 本文件依赖 `std` + `runquiry-core`，可在任何平台编译；Linux 上经
//! `#[path = "../src/windows/unsupported.rs"]` 由 `tests/windows_unsupported.rs`
//! 直接执行。parity §10：Windows 使用文件共享模式而非 POSIX 式锁，无全系统
//! 锁枚举 API，文件锁整类不可安全提供——一律表达为
//! [`CapabilityStatus::Unsupported`]，不返回伪数据、不经命令行模拟。
//! 进程控制的关闭类（terminate/kill/kill-tree）已由 `controller.rs` 实现；
//! 暂停/恢复/renice 在 Windows 仍不提供（用户裁决 2026-09-12：仅关闭类）。
//! 原因键字符串必须保持稳定，供 UI/i18n 关联。

use std::path::Path;

use runquiry_core::{DiagnosticCode, DiagnosticIssue, InspectError, Inspection};

/// FileInventory 能力不可用的稳定原因键（CapabilityStatus 与诊断共用）。
pub(super) const FILE_LOCKS_REASON: &str = "Windows 平台不提供文件锁枚举（parity §10）";

/// Windows 关闭类之外动作（pause/resume/renice）不可用的稳定原因键
/// （`ProcessController::action_capability` 逐动作返回）。
pub(super) const KILL_ONLY_REASON: &str =
    "Windows 平台仅支持关闭类操作（terminate/kill/kill-tree）；暂停/恢复/renice 不可用";

/// File Locks 工作区的固定失败清单（`data = None` + Unsupported 诊断，
/// 不返回伪数据）。
#[must_use]
pub(super) fn file_locks_failed_list() -> Inspection<Vec<runquiry_core::FileInventoryEntry>> {
    Inspection::failed(vec![DiagnosticIssue::new(
        DiagnosticCode::Unsupported,
        String::from(FILE_LOCKS_REASON),
    )])
}

/// 按路径的持有者查询：同为固定失败（Windows 文件目标为 Unsupported）。
#[must_use]
pub(super) fn file_locks_failed_holders(
    path: &Path,
) -> Inspection<Vec<runquiry_core::FileInventoryEntry>> {
    let _unused = path;
    file_locks_failed_list()
}

/// Windows 不支持动作的固定错误（正常流程不应到达——UI 依据逐动作能力态
/// 先行隐藏入口；防御性返回保证误用也不产生副作用）。
#[must_use]
pub(super) fn unsupported_action_error() -> InspectError {
    InspectError::Unsupported {
        reason: String::from(KILL_ONLY_REASON),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        KILL_ONLY_REASON, file_locks_failed_holders, file_locks_failed_list,
        unsupported_action_error,
    };
    use runquiry_core::{CapabilityStatus, DiagnosticCode, FileInventory, Inspection};
    struct UnsupportedFileInventory;

    impl FileInventory for UnsupportedFileInventory {
        fn capability(&self) -> CapabilityStatus {
            CapabilityStatus::Unsupported(String::from(super::FILE_LOCKS_REASON))
        }

        fn list(&self) -> Inspection<Vec<runquiry_core::FileInventoryEntry>> {
            file_locks_failed_list()
        }

        fn holders(
            &self,
            path: &std::path::Path,
        ) -> Inspection<Vec<runquiry_core::FileInventoryEntry>> {
            file_locks_failed_holders(path)
        }
    }

    #[test]
    fn reason_keys_are_stable() {
        assert_eq!(
            super::FILE_LOCKS_REASON,
            "Windows 平台不提供文件锁枚举（parity §10）"
        );
        assert_eq!(
            KILL_ONLY_REASON,
            "Windows 平台仅支持关闭类操作（terminate/kill/kill-tree）；暂停/恢复/renice 不可用"
        );
    }

    #[test]
    fn file_locks_fail_without_data_and_with_unsupported_code() {
        let inspection = UnsupportedFileInventory.list();
        assert_eq!(inspection.data, None);
        assert!(inspection.has_issues());
        assert!(
            inspection
                .issues
                .iter()
                .all(|issue| issue.code() == DiagnosticCode::Unsupported)
        );
        let holders = UnsupportedFileInventory
            .holders(std::path::Path::new("C:\\opt\\runquiry-fixtures\\fxt.lock"));
        assert_eq!(holders.data, None);
        assert_eq!(
            UnsupportedFileInventory.capability().reason(),
            Some(super::FILE_LOCKS_REASON)
        );
    }

    #[test]
    fn unsupported_action_error_is_defensive_unsupported() {
        let error = unsupported_action_error();
        assert_eq!(error.code(), "unsupported");
        assert!(
            matches!(error, runquiry_core::InspectError::Unsupported { ref reason } if reason == KILL_ONLY_REASON)
        );
    }
}
