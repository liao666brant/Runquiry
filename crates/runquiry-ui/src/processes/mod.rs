//! Processes 工作区模块边界。

mod action;
mod command;
mod detail;
mod model;
mod query;
mod redaction;
mod surface;
mod table;

pub use command::{ProcessActionShortcut, ProcessCommand};
pub use detail::{AnalysisSections, DetailSection};
pub use model::{DetailRequest, ProcessRows, ProcessesState, SelectionChange};
pub use query::{QueryOutcome, TargetKind};
pub use redaction::{DetailPrivacySession, RedactedArgument, RedactedEnvironment};
pub use surface::{SurfaceSnapshot, SurfaceState};
pub use table::{ProcessTable, ProcessTableDelegate};

#[cfg(test)]
mod action_tests;
#[cfg(test)]
mod tests;
/// 动作菜单快捷键只在菜单已打开且焦点不属于输入控件时生效。
pub(crate) const fn accepts_action_shortcut(menu_open: bool, input_focused: bool) -> bool {
    menu_open && !input_focused
}
pub use action::{ActionCompletion, ActionRequest, ProcessActionFlow, SuccessDisposition};
