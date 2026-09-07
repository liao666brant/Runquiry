//! 平台后端无法构造时的诚实能力边界。

use std::sync::Arc;

use runquiry_core::{
    Analysis, CapabilityStatus, InspectError, Inspection, ProcessAction, ProcessIdentity,
    QueryTarget, Resolution,
};
use runquiry_ui::WorkspaceId;
use runquiry_ui::backend::{InvestigationTarget, WorkspaceBackend, WorkspaceSnapshot};

/// 无法构造平台后端时供窗口显示不可用状态。
#[derive(Debug)]
pub struct UnavailableBackend {
    reason: String,
}

impl UnavailableBackend {
    /// 保存构造失败原因。
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl WorkspaceBackend for UnavailableBackend {
    fn load(&self, workspace: WorkspaceId) -> WorkspaceSnapshot {
        let capability = CapabilityStatus::Unavailable(self.reason.clone());
        match workspace {
            WorkspaceId::Processes => WorkspaceSnapshot::Processes {
                capability,
                inspection: Inspection::complete(Arc::default()),
            },
            WorkspaceId::Ports => WorkspaceSnapshot::Ports {
                capability,
                inspection: Inspection::complete(Arc::default()),
            },
            WorkspaceId::Containers => WorkspaceSnapshot::Containers {
                capability,
                inspection: Inspection::complete(Arc::default()),
            },
            WorkspaceId::FileLocks => WorkspaceSnapshot::FileLocks {
                capability,
                inspection: Inspection::complete(Arc::default()),
            },
        }
    }

    fn resolve(&self, _: &QueryTarget) -> Result<Resolution<InvestigationTarget>, InspectError> {
        Err(InspectError::Unsupported {
            reason: self.reason.clone(),
        })
    }

    fn analyze(&self, _: &ProcessIdentity) -> Result<Inspection<Analysis>, InspectError> {
        Err(InspectError::Unsupported {
            reason: self.reason.clone(),
        })
    }

    fn process_control_capability(&self) -> CapabilityStatus {
        // 与 load 一致：无法构造平台是环境不可用，不是平台不支持。
        CapabilityStatus::Unavailable(self.reason.clone())
    }

    fn execute_process_action(
        &self,
        _: &ProcessIdentity,
        _: ProcessAction,
    ) -> Result<(), InspectError> {
        Err(InspectError::Unsupported {
            reason: self.reason.clone(),
        })
    }
}
