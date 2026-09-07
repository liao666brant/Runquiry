//! Windows Unsupported 能力语义的 Linux 编译入口（见 `windows_utf16.rs`
//! 头注释）。

#[path = "../src/windows/unsupported.rs"]
mod unsupported;

#[test]
fn unsupported_module_compiles_and_reasons_are_stable() {
    assert_eq!(
        unsupported::file_locks_failed_list().issues.len(),
        1
    );
    assert_eq!(
        unsupported::control_error().code(),
        "unsupported"
    );
}