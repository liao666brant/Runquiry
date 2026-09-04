//! 告警规则的集成测试入口；按契约、顺序、状态与上下文拆分以保持可审阅性。

#[path = "warnings/context.rs"]
mod context;
#[path = "warnings/contracts.rs"]
mod contracts;
#[path = "warnings/environment.rs"]
mod environment;
#[path = "warnings/order.rs"]
mod order;
#[path = "warnings/state.rs"]
mod state;
#[path = "warnings/support.rs"]
pub mod support;
