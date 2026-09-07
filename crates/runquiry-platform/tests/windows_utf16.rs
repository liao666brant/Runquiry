//! Windows 纯解析模块在 Linux 上的编译与执行入口。
//!
//! `src/windows/` 下的纯模块只依赖 `std` + `runquiry-core`（无 OS / FFI
//! 依赖），经 `#[path]` 引入后其内联 `#[cfg(test)]` 测试在 Linux 直接运行；
//! `cfg(windows)` 代码（ffi / peb_reader / 端口实现）本机不编译，由主代理
//! 交叉编译检查。
// utf16 的边界常量由 peb/fields 消费，本目标不包含该模块，常量在此按
// dead_code 告警；测试入口仅验证 utf16 自身，放行。
#![allow(dead_code)]

#[path = "../src/windows/utf16.rs"]
mod utf16;

#[test]
fn utf16_module_compiles_and_smoke_decodes() {
    let units: Vec<u16> = "fxt".encode_utf16().collect();
    assert_eq!(utf16::decode_lossy_units(&units), "fxt");
}
