//! 合成 fixture 消费测试入口；按进程、运行时、socket 与文件锁语义拆分。

mod support;

#[path = "fixtures_load/locks.rs"]
mod locks;
#[path = "fixtures_load/metadata.rs"]
pub mod metadata;
#[path = "fixtures_load/processes.rs"]
mod processes;
#[path = "fixtures_load/runtimes.rs"]
mod runtimes;
#[path = "fixtures_load/sockets.rs"]
mod sockets;
