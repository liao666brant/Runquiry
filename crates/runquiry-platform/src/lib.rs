//! Runquiry 平台采集器、容器运行时与进程控制。
//!
//! 为 [`runquiry_core`] 的平台端口 trait 提供三平台实现；UI 不得绕过本层
//! 直接读取 `/proc`、调用 Win32 API 或运行 lsof/容器 CLI。
//!
//! 模块编译范围：
//! * [`command`] / [`container`]：跨平台（经 `CommandRunner` 的外部命令边界）；
//! * [`linux`]：仅 `target_os = "linux"`；
//! * [`macos`]：仅 `target_os = "macos"`（C1，模块 06）；
//! * [`windows`]：仅 `target_os = "windows"`（C2，模块 07）。
//!
// 锁定依赖树既有的 syn 2.0.119 / 3.0.4 双版本（GPUI 传递依赖引入，Cargo.lock
// 未变）在本 crate 作为叶子时触发该 cargo 警告；传递依赖版本不受本 crate
// 控制，与 tests/fixtures/README.md §6 记录的既有豁免约定一致。
#![allow(clippy::multiple_crate_versions)]

pub mod command;
pub mod container;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;
