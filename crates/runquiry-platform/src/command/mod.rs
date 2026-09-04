//! 受限外部命令执行器（B3）：`CommandRunner` 的生产实现。
//!
//! 只接受程序名 + 独立 argv，绝不经过 shell；stdout/stderr 并发读取、
//! 分别限幅，超时/超限终止并回收子进程。子模块由本目录内代码自组织。

mod runner;

pub use runner::{CommandFailure, StdCommandRunner};
