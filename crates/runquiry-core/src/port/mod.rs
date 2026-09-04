//! 平台端口：core 定义的同步 trait，由 runquiry-platform 实现。
//!
//! 全部为同步接口（无 `async fn`、不引入 async runtime），由上层后台执行器调度；
//! 按职责单一拆分，不建胖接口。平台能力一律通过
//! [`CapabilityStatus`](crate::model::capability::CapabilityStatus) 显式暴露。

pub mod command;
pub mod container;
pub mod file;
pub mod network;
pub mod process;
pub mod source;
