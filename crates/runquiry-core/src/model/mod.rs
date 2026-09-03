//! 公共领域类型：标识、目标、进程身份、部分成功与错误语义。
//!
//! 所有类型仅依赖 `serde`，不依赖 GPUI 或操作系统实现；序列化用于测试 fixture，
//! 不定义持久化数据库格式。

pub mod capability;
pub mod diagnostic;
pub mod error;
pub mod ids;
pub mod inspection;
pub mod process;
pub mod target;
