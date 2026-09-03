//! Runquiry GPUI 界面层：设计系统（A4）与后续工作区。
//!
//! 当前内容：
//! - [`theme`]：Runquiry 浅/深主题（唯一允许出现原始色值的地方）。
//! - [`state`]：五种数据状态的语义定义。
//! - [`state_view`]：统一的状态呈现组件。
//! - [`locale`]：A4 阶段的最小双语字典（B4 将迁移到 rust-i18n）。

pub mod locale;
pub mod state;
pub mod state_view;
pub mod theme;

pub use locale::{Dict, Lang};
pub use state::DataState;
pub use state_view::StateView;
