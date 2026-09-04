//! Linux 系统采集适配器（B2）。
//!
//! 仅在 `target_os = "linux"` 下参与编译；为 core 端口提供真实只读实现：
//! * [`LinuxPlatform`]：进程基线与详情（[`ProcessInventory`] /
//!   [`ProcessDetailsProvider`]）、网络与 Socket（[`NetworkInventory`]）、
//!   文件锁（[`FileInventory`]）、来源证据（[`SourceEvidenceProvider`]）。
//!
//! 全部 `/proc` 读取经可注入根目录（[`LinuxPlatform::with_injected`]），
//! 生产实例为 `/proc`；部分成功语义：单点失败只追加诊断，不返回空集合
//! 冒充成功，不自动提权。
//!
//! [`ProcessInventory`]: runquiry_core::ProcessInventory
//! [`ProcessDetailsProvider`]: runquiry_core::ProcessDetailsProvider
//! [`NetworkInventory`]: runquiry_core::NetworkInventory
//! [`FileInventory`]: runquiry_core::FileInventory
//! [`SourceEvidenceProvider`]: runquiry_core::SourceEvidenceProvider

mod capabilities;
mod details;
mod fdscan;
mod locks;
mod network;
mod process;
mod procfs;
mod source;

pub use process::LinuxPlatform;
