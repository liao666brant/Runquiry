//! Processes 工作区模块边界。

mod command;
mod detail;
mod model;
mod query;
mod redaction;
mod surface;
mod table;

pub use command::ProcessCommand;
pub use detail::{AnalysisSections, DetailSection};
pub use model::{DetailRequest, ProcessRows, ProcessesState, SelectionChange};
pub use query::{QueryOutcome, TargetKind};
pub use redaction::{DetailPrivacySession, RedactedArgument, RedactedEnvironment};
pub use surface::{SurfaceSnapshot, SurfaceState};
pub use table::{ProcessTable, ProcessTableDelegate};

#[cfg(test)]
mod tests;
