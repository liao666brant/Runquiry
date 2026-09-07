//! Windows 错误映射的 Linux 编译入口（见 `windows_utf16.rs` 头注释）。

#[path = "../src/windows/winerror.rs"]
mod winerror;

#[test]
fn winerror_module_compiles_and_smoke_maps() {
    let issue = winerror::diagnostic_for(winerror::Win32Error(winerror::ERROR_ACCESS_DENIED), "表");
    assert_eq!(issue.code(), runquiry_core::DiagnosticCode::PermissionDenied);
}