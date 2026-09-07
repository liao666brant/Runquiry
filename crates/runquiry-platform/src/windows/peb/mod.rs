//! PEB / RTL_USER_PROCESS_PARAMETERS 的纯布局、字段提取与读取计划（无 OS 依赖）。
//!
//! 本目录依赖 `std`（经 `utf16` 常量），可在任何平台编译；Linux 上经
//! `#[path = "../src/windows/peb/mod.rs"]` 由 `tests/windows_peb.rs` 直接执行
//! （配合同模块树的 `utf16`）。偏移为公开 ABI 常量（与 witr
//! `peb_windows.go` 的 `rtlUserProcessParameters` 布局一致；32 位布局对应
//! `RTL_USER_PROCESS_PARAMETERS32`）。本目录只产出「读什么、读多长」的
//! 有界计划与校验结果，不执行任何系统调用——真实读取集中在
//! `ffi` / `peb_reader`（仅 Windows 编译）。
//!
//! 结构：[`layout`](self::layout)（偏移常量与读取计划）、
//! [`fields`](self::fields)（UNICODE_STRING 校验与字段 / 环境指针提取）。

mod fields;
mod layout;

// 重导出供生产消费方（peb_reader）与测试按需取用；不同消费方使用的子集
// 不同，统一放行未使用导入，避免逐目标告警。
#[allow(unused_imports)]
pub(super) use fields::{
    PebFieldError, RemoteString, env_block_limits, environment_pointer, remote_string_field,
};
#[cfg(test)]
#[allow(unused_imports)]
pub(super) use fields::{UnicodeStringError, validate_unicode_string};
#[cfg(test)]
#[allow(unused_imports)]
pub(super) use layout::{PEB32_PARAMS_PTR_OFFSET, PEB64_PARAMS_PTR_OFFSET, PebLayout, ReadPlan};
#[allow(unused_imports)]
pub(super) use layout::{build_plan, extract_pointer, layout_for};
