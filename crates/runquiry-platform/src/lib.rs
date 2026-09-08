//! Runquiry 平台采集器、容器运行时与进程控制。
//!
//! 为 [`runquiry_core`] 的平台端口 trait 提供 Linux 与 Windows 实现；UI 不得
//! 绕过本层直接读取 `/proc`、调用 Win32 API 或运行外部命令/容器 CLI。
//!
//! 模块编译范围：
//! * [`command`] / [`container`]：跨平台（经 `CommandRunner` 的外部命令边界）；
//! * [`linux`]：仅 `target_os = "linux"`；
//! * [`windows`]：仅 `target_os = "windows"`（C2，模块 07）。
//!
// 锁定依赖树既有的 syn 2.0.119 / 3.0.4 双版本（GPUI 传递依赖引入，Cargo.lock
// 未变）在本 crate 作为叶子时触发该 cargo 警告；传递依赖版本不受本 crate
// 控制，与 tests/fixtures/README.md §6 记录的既有豁免约定一致。
#![allow(clippy::multiple_crate_versions)]

// 目标平台门禁：macOS 支持已移出 v1 范围，非 Linux/Windows 目标在此显式报错，
// 而不是让下游落到「找不到平台类型」的晦涩编译失败。
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
compile_error!("Runquiry 仅支持 Linux 与 Windows 目标平台");

pub mod command;
pub mod container;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "windows")]
pub mod windows;
