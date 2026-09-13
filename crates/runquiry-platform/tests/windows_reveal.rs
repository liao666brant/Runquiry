//! Windows 可执行文件定位纯逻辑在 Linux 上的编译与执行入口。
//!
//! `src/windows/reveal.rs` 只依赖 `std`（无 OS / FFI 依赖），经 `#[path]`
//! 引入后其内联 `#[cfg(test)]` 测试在 Linux 直接运行；实际的
//! `ShellExecuteW` 调用在 `windows::ffi::shell`，本机不编译。
// `#[path]` 目标把子模块提到 crate 根，`pub(super)` 成为对外可见，触发
// nursery 的 redundant_pub_crate——与生产可见性要求不冲突，这里放行。
#![allow(clippy::redundant_pub_crate)]

#[path = "../src/windows/reveal.rs"]
mod reveal;

#[test]
fn reveal_module_compiles_and_select_parameter_is_quoted() {
    assert_eq!(
        reveal::select_parameter(std::path::Path::new(r"C:\fxt\app.exe")),
        "/select,\"C:\\fxt\\app.exe\""
    );
    assert!(!reveal::is_failure(42));
    assert!(reveal::is_failure(2));
}
