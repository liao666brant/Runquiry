//! Runquiry GPUI 界面层。
//!
//! - [`theme`]：Runquiry 浅/深主题（唯一允许出现原始色值的地方）。
//! - [`backend`]：应用装配层实现的同步只读采集边界。
//! - [`state`] + [`state_view`]：六态数据状态与统一状态呈现。
//! - [`locale`]：语言枚举与 rust-i18n 初始化（文案键在 `locales/`，en 兜底）。
//! - [`session`]：每工作区会话状态（LoadState、generation、选择、排序、筛选）。
//! - [`debounce`]：500ms 详情加载防抖（注入时钟，纯逻辑）。
//! - [`shell`]：产品应用壳层（侧栏/工具栏/主数据区/详情区/状态栏）。

pub mod backend;
pub mod debounce;
pub mod locale;
pub mod processes;
pub mod session;
pub mod shell;
pub mod state;
pub mod state_view;
pub mod theme;
pub mod workspaces;

// rust-i18n：本 crate 的翻译键来自 `locales/`，en 为兜底；启动时在
// gpui-component 初始化前调用 [`locale::extend_component_translations`] 一次，
// 让组件内置文案也尊重我们的覆盖键。
rust_i18n::i18n!("locales", fallback = "en");

pub use debounce::DetailDebounce;
pub use locale::{Lang, set_language, state_copy, state_name, tr};
pub use session::{AppSession, WorkspaceId, WorkspaceSession};
pub use shell::{AppShell, ShellEvent, ShellStartup};
pub use state::DataState;
pub use state_view::StateView;
