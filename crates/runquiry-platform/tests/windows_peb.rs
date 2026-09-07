//! Windows PEB 纯布局与读取计划的 Linux 编译入口（见 `windows_utf16.rs`
//! 头注释）。
// utf16 的边界常量与 fields 的 env_block_limits 由 cfg(windows) 生产模块
// 消费，本目标不含。
#![allow(dead_code)]

#[path = "../src/windows/utf16.rs"]
mod utf16;

#[path = "../src/windows/peb/mod.rs"]
mod peb;

#[test]
fn peb_module_compiles_and_smoke_builds_plan() {
    let layout = peb::PebLayout::win64();
    let plan = peb::build_plan(&layout, 0x0000_7FF6_0000_0000);
    assert_eq!(plan.params_ptr, (0x0000_7FF6_0000_0020, 8));
}
