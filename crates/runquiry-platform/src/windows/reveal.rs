//! Windows 可执行文件定位的纯逻辑（explorer `/select` 参数与 `ShellExecute`
//! 返回值语义）。
//!
//! 本文件只依赖 `std`，可在任何平台编译；Linux 上经
//! `#[path = "../src/windows/reveal.rs"]` 由 `tests/windows_reveal.rs` 直接
//! 执行。实际的 `ShellExecuteW` 调用在 `windows::ffi::shell`。

use std::path::Path;

/// explorer.exe 的定位参数：`/select,"<path>"`。
///
/// Windows 文件名不得含 `"`（Win32 保留字符），因此以双引号包裹路径不会被
/// 路径内容突破引号，无参数注入面。
#[must_use]
pub(super) fn select_parameter(path: &Path) -> String {
    format!("/select,\"{}\"", path.display())
}

/// `ShellExecuteW` 返回值是否为失败。
///
/// 契约：返回值大于 32 视为成功（HINSTANCE 句柄），`0..=32` 为 `SE_ERR_*`
/// / `ERROR_*` 错误码。
#[must_use]
pub(super) const fn is_failure(instance: isize) -> bool {
    instance <= 32
}

/// `SE_ERR_FNF`：文件不存在。
const SE_ERR_FNF: isize = 2;
/// `SE_ERR_PNF`：路径不存在。
const SE_ERR_PNF: isize = 3;
/// `SE_ERR_ACCESSDENIED`：拒绝访问。
const SE_ERR_ACCESSDENIED: isize = 5;

/// `ShellExecuteW` 失败码的类别；调用方据此映射领域结论。
///
/// 只有 [`is_failure`] 为真的码才需要分类：把「路径不可得」与「文件管理器
/// 调用失败」区分开，前者是定位目标的正常未命中，不是平台能力缺失。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RevealFailure {
    /// 路径不存在（`SE_ERR_FNF` / `SE_ERR_PNF`）。
    Missing,
    /// 路径存在但拒绝访问（`SE_ERR_ACCESSDENIED`）。
    AccessDenied,
    /// 其余 `SE_ERR_*` / `ERROR_*`（文件管理器调用本身失败）。
    Other,
}

/// 分类 `ShellExecuteW` 失败码。
#[must_use]
pub(super) const fn failure_kind(code: isize) -> RevealFailure {
    match code {
        SE_ERR_FNF | SE_ERR_PNF => RevealFailure::Missing,
        SE_ERR_ACCESSDENIED => RevealFailure::AccessDenied,
        _ => RevealFailure::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::{RevealFailure, failure_kind, is_failure, select_parameter};
    use std::path::Path;

    #[test]
    fn select_parameter_quotes_path_without_injection_surface() {
        assert_eq!(
            select_parameter(Path::new(r"C:\Program Files\fxt-app.exe")),
            "/select,\"C:\\Program Files\\fxt-app.exe\""
        );
    }

    #[test]
    fn shell_success_threshold_matches_win32_contract() {
        assert!(is_failure(0));
        assert!(is_failure(32));
        assert!(!is_failure(33));
        assert!(!is_failure(42));
    }

    #[test]
    fn failure_kinds_separate_missing_path_from_shell_failure() {
        // SE_ERR_FNF / SE_ERR_PNF：路径不存在。
        assert_eq!(failure_kind(2), RevealFailure::Missing);
        assert_eq!(failure_kind(3), RevealFailure::Missing);
        // SE_ERR_ACCESSDENIED：路径存在但拒绝访问。
        assert_eq!(failure_kind(5), RevealFailure::AccessDenied);
        // 其余码（如 SE_ERR_OOM=8）：文件管理器调用本身失败。
        assert_eq!(failure_kind(8), RevealFailure::Other);
        assert_eq!(failure_kind(31), RevealFailure::Other);
    }
}
