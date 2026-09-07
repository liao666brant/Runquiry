//! Windows 错误映射的 Linux 编译入口（见 `windows_utf16.rs` 头注释）。
// winerror 的部分常量与 is_target_gone_or_invalid 仅被 cfg(windows) 生产
// 模块消费，本目标不含。
#![allow(dead_code)]

#[path = "../src/windows/winerror.rs"]
mod winerror;

#[test]
fn winerror_module_compiles_and_smoke_maps() {
    let issue = winerror::diagnostic_for(winerror::Win32Error(winerror::ERROR_ACCESS_DENIED), "表");
    assert_eq!(
        issue.code(),
        runquiry_core::DiagnosticCode::PermissionDenied
    );
}
