//! UI 与平台装配层之间的只读采集边界。

use std::sync::Arc;

use runquiry_core::{
    Analysis, CapabilityStatus, ContainerSummary, DiagnosticIssue, FileInventoryEntry, Generation,
    InspectError, Inspection, OpenPortEntry, Pid, ProcessAction, ProcessIdentity, ProcessSummary,
    QueryTarget, Resolution,
};

use crate::WorkspaceId;

/// 容器工作区的只读行；宿主 PID 只在平台归属验证成功后出现。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContainerSnapshotRow {
    /// 运行时清单数据。
    pub summary: ContainerSummary,
    /// 已验证宿主 PID。
    pub verified_host_pid: Option<runquiry_core::Pid>,
}

/// 端口工作区的领域条目与同次平台采集得到的进程名。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortSnapshotRow {
    /// 网络端口条目。
    pub entry: OpenPortEntry,
    /// 属主进程名；无权限、已退出或无属主时为空。
    pub process: Option<String>,
}

/// 容器目标无法安全映射到宿主进程时的真实容器详情。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContainerInvestigation {
    /// 运行时提供的容器状态。
    pub summary: ContainerSummary,
    /// 仅在归属验证成功时出现；缺失不会把容器误报为未找到。
    pub verified_host_pid: Option<Pid>,
    /// 解析、清单与宿主 PID 验证产生的部分诊断。
    pub issues: Arc<[DiagnosticIssue]>,
}

/// 调查目标可能进入完整进程分析，也可能保留为容器自身详情。
#[derive(Clone, Debug)]
pub enum InvestigationTarget {
    /// 可安全分析的进程身份。
    Process(ProcessIdentity),
    /// 无可验证宿主 PID 的容器 fallback。
    Container(ContainerInvestigation),
}

/// 一次工作区采集结果；枚举保证结果不会被应用到错误页面。
#[derive(Clone, Debug)]
pub enum WorkspaceSnapshot {
    /// 进程清单。
    Processes {
        /// 平台能力。
        capability: CapabilityStatus,
        /// 数据与诊断。
        inspection: Inspection<Arc<[ProcessSummary]>>,
    },
    /// 端口清单。
    Ports {
        /// 平台能力。
        capability: CapabilityStatus,
        /// 数据与诊断。
        inspection: Inspection<Arc<[PortSnapshotRow]>>,
    },
    /// 容器清单。
    Containers {
        /// 平台能力。
        capability: CapabilityStatus,
        /// 数据与诊断。
        inspection: Inspection<Arc<[ContainerSnapshotRow]>>,
    },
    /// 文件清单。
    FileLocks {
        /// 平台能力。
        capability: CapabilityStatus,
        /// 数据与诊断。
        inspection: Inspection<Arc<[FileInventoryEntry]>>,
    },
}

impl WorkspaceSnapshot {
    /// 快照所属工作区。
    pub const fn workspace(&self) -> WorkspaceId {
        match self {
            Self::Processes { .. } => WorkspaceId::Processes,
            Self::Ports { .. } => WorkspaceId::Ports,
            Self::Containers { .. } => WorkspaceId::Containers,
            Self::FileLocks { .. } => WorkspaceId::FileLocks,
        }
    }
}

/// 后台结果的工作区与代际门；两个坐标都匹配才能应用。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkspaceResultGate {
    workspace: WorkspaceId,
    generation: Generation,
}

impl WorkspaceResultGate {
    /// 建立请求门。
    pub const fn new(workspace: WorkspaceId, generation: Generation) -> Self {
        Self {
            workspace,
            generation,
        }
    }

    /// 请求所属工作区。
    pub const fn workspace(self) -> WorkspaceId {
        self.workspace
    }

    /// 请求代际。
    pub const fn generation(self) -> Generation {
        self.generation
    }

    /// 判断回调是否属于当前请求。
    pub fn accepts(self, workspace: WorkspaceId, generation: Generation) -> bool {
        self.workspace == workspace && !generation.is_stale(self.generation)
    }

    /// 同时核对快照路由与真实会话代际。
    pub fn accepts_session(
        self,
        session: &crate::AppSession,
        snapshot: &WorkspaceSnapshot,
    ) -> bool {
        self.accepts(
            snapshot.workspace(),
            session.session(self.workspace).generation,
        )
    }
}

/// App 装配层实现的同步只读后端。
///
/// 壳层只在 GPUI 后台执行器调用这些方法，UI crate 不依赖操作系统实现。
pub trait WorkspaceBackend: Send + Sync {
    /// 采集一个工作区。
    fn load(&self, workspace: WorkspaceId) -> WorkspaceSnapshot;

    /// 把显式类型目标解析为进程身份。
    ///
    /// # Errors
    /// 目标无效、不可见或采集失败时返回领域错误。
    fn resolve(
        &self,
        target: &QueryTarget,
    ) -> Result<Resolution<InvestigationTarget>, InspectError>;

    /// 执行完整只读分析。
    ///
    /// # Errors
    /// 目标退出、PID 复用或关键采集失败时返回领域错误。
    fn analyze(&self, identity: &ProcessIdentity) -> Result<Inspection<Analysis>, InspectError>;

    /// 当前平台的进程控制能力；默认后端必须明确保持安全禁用。
    fn process_control_capability(&self) -> CapabilityStatus {
        CapabilityStatus::Unsupported(String::from("process control is not configured"))
    }

    /// 执行已经过 UI 两步确认的进程动作。
    ///
    /// # Errors
    /// 权限不足、身份变化、目标退出或平台不支持时返回领域错误。
    fn execute_process_action(
        &self,
        _: &ProcessIdentity,
        _: ProcessAction,
    ) -> Result<(), InspectError> {
        Err(InspectError::Unsupported {
            reason: String::from("process control is not configured"),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use runquiry_core::{
        Analysis, Generation, InspectError, Inspection, ProcessIdentity, QueryTarget, Resolution,
    };

    use super::{WorkspaceBackend, WorkspaceResultGate, WorkspaceSnapshot};
    use crate::{AppSession, WorkspaceId};

    struct FakeBackend;

    impl WorkspaceBackend for FakeBackend {
        fn load(&self, _: WorkspaceId) -> WorkspaceSnapshot {
            WorkspaceSnapshot::Ports {
                capability: runquiry_core::CapabilityStatus::Supported,
                inspection: Inspection::complete(Arc::default()),
            }
        }

        fn resolve(
            &self,
            _: &QueryTarget,
        ) -> Result<Resolution<super::InvestigationTarget>, InspectError> {
            Err(InspectError::Unsupported {
                reason: String::from("unused by this fake"),
            })
        }

        fn analyze(&self, _: &ProcessIdentity) -> Result<Inspection<Analysis>, InspectError> {
            Err(InspectError::Unsupported {
                reason: String::from("unused by this fake"),
            })
        }
    }

    #[test]
    fn result_gate_rejects_other_workspace_and_stale_generation() {
        let gate = WorkspaceResultGate::new(WorkspaceId::Ports, Generation::first().next());

        assert!(!gate.accepts(WorkspaceId::Processes, Generation::first().next()));
        assert!(!gate.accepts(WorkspaceId::Ports, Generation::first()));
        assert!(gate.accepts(WorkspaceId::Ports, Generation::first().next()));
    }

    #[test]
    fn fake_backend_result_is_rejected_after_real_session_interactions() {
        let backend = FakeBackend;
        let mut session = AppSession::new();
        session.switch_workspace(WorkspaceId::Ports);
        let generation = session
            .active_session_mut()
            .try_refresh()
            .unwrap_or_default();
        let gate = WorkspaceResultGate::new(WorkspaceId::Ports, generation);
        let snapshot = backend.load(WorkspaceId::Ports);

        session
            .active_session_mut()
            .set_filter(Some(String::from("443")));

        assert!(!gate.accepts_session(&session, &snapshot));
        session
            .active_session_mut()
            .abort_refresh(gate.generation());

        let selection_gate = WorkspaceResultGate::new(
            WorkspaceId::Ports,
            session
                .active_session_mut()
                .try_refresh()
                .unwrap_or_default(),
        );
        session
            .active_session_mut()
            .select(Some(String::from("tcp:127.0.0.1:443")));
        assert!(!selection_gate.accepts_session(&session, &snapshot));

        session
            .active_session_mut()
            .abort_refresh(selection_gate.generation());
        let mode_gate = WorkspaceResultGate::new(
            WorkspaceId::Ports,
            session
                .active_session_mut()
                .try_refresh()
                .unwrap_or_default(),
        );
        session.active_session_mut().invalidate_context();
        assert!(!mode_gate.accepts_session(&session, &snapshot));

        session
            .active_session_mut()
            .abort_refresh(mode_gate.generation());
        let sort_gate = WorkspaceResultGate::new(
            WorkspaceId::Ports,
            session
                .active_session_mut()
                .try_refresh()
                .unwrap_or_default(),
        );
        session
            .active_session_mut()
            .set_sort(Some(crate::session::SortOrder {
                column: String::from("port"),
                ascending: true,
            }));
        assert!(!sort_gate.accepts_session(&session, &snapshot));
    }

    #[test]
    fn default_process_control_seam_is_explicitly_unsupported() {
        let backend = FakeBackend;
        let identity = ProcessIdentity::new(
            runquiry_core::Pid::MIN,
            Some(std::time::SystemTime::UNIX_EPOCH),
            None,
        );

        assert!(matches!(
            backend.process_control_capability(),
            runquiry_core::CapabilityStatus::Unsupported(_)
        ));
        assert!(matches!(
            backend.execute_process_action(&identity, runquiry_core::ProcessAction::Terminate),
            Err(InspectError::Unsupported { .. })
        ));
    }
}
