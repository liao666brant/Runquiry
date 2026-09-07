//! Windows 不支持能力的稳定原因键与固定失败结果（纯逻辑，无 OS 依赖）。
//!
//! 本文件依赖 `std` + `runquiry-core`，可在任何平台编译；Linux 上经
//! `#[path = "../src/windows/unsupported.rs"]` 由 `tests/windows_unsupported.rs`
//! 直接执行。parity §10：Windows 使用文件共享模式而非 POSIX 式锁，无全系统
//! 锁枚举 API；进程操作（terminate/kill/pause/resume/renice）整类不可安全
//! 提供——两者一律表达为 [`CapabilityStatus::Unsupported`]，不返回伪数据、
//! 不经命令行模拟。原因键字符串必须保持稳定，供 UI/i18n 关联。

use std::path::Path;

use runquiry_core::{DiagnosticCode, DiagnosticIssue, InspectError, Inspection};

/// FileInventory 能力不可用的稳定原因键（CapabilityStatus 与诊断共用）。
pub const FILE_LOCKS_REASON: &str = "Windows 平台不提供文件锁枚举（parity §10）";

/// ProcessController 能力不可用的稳定原因键（覆盖 terminate / kill /
/// pause / resume / renice 整类动作）。
pub const PROCESS_CONTROL_REASON: &str =
    "Windows 平台不支持进程控制操作（terminate/kill/pause/resume/renice，parity §10）";

/// File Locks 工作区的固定失败清单（`data = None` + Unsupported 诊断，
/// 不返回伪数据）。
#[must_use]
pub fn file_locks_failed_list() -> Inspection<Vec<runquiry_core::FileInventoryEntry>> {
    Inspection::failed(vec![DiagnosticIssue::new(
        DiagnosticCode::Unsupported,
        String::from(FILE_LOCKS_REASON),
    )])
}

/// 按路径的持有者查询：同为固定失败（Windows 文件目标为 Unsupported）。
#[must_use]
pub fn file_locks_failed_holders(path: &Path) -> Inspection<Vec<runquiry_core::FileInventoryEntry>> {
    let _unused = path;
    file_locks_failed_list()
}

/// 进程操作的固定错误（正常流程不应到达——UI 依据能力态先行禁用；
/// 防御性返回保证误用也不产生副作用）。
#[must_use]
pub fn control_error() -> InspectError {
    InspectError::Unsupported {
        reason: String::from(PROCESS_CONTROL_REASON),
    }
}

#[cfg(test)]
mod tests {
    use super::{PROCESS_CONTROL_REASON, file_locks_failed_holders, control_error, file_locks_failed_list};
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
            PROCESS_CONTROL_REASON,
            "Windows 平台不支持进程控制操作（terminate/kill/pause/resume/renice，parity §10）"
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
        let holders = UnsupportedFileInventory.holders(std::path::Path::new("C:\\opt\\runquiry-fixtures\\fxt.lock"));
        assert_eq!(holders.data, None);
        assert_eq!(
            UnsupportedFileInventory.capability().reason(),
            Some(super::FILE_LOCKS_REASON)
        );
    }

    #[test]
    fn control_execute_is_defensive_unsupported_error() {
        let error = control_error();
        assert_eq!(error.code(), "unsupported");
    }
}