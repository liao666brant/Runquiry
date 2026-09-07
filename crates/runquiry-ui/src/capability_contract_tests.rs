//! 三平台风格假后端共用同一套 UI 能力契约断言（模块 05 C3）。
//!
//! 假后端只合成 `WorkspaceSnapshot` 与控制能力，状态语义与动作判定全部走
//! 生产转换（`LoadPresentation::apply`、`ProcessActionFlow`），不测 mock 自身。
//! 平台风格对齐 Batch 6 各平台真实能力返回；纯 `#[test]`，不依赖 GPUI 实体。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use runquiry_core::{
    Analysis, CapabilityStatus, DiagnosticCode, DiagnosticIssue, FileInventoryEntry, Generation,
    InspectError, Inspection, Pid, ProcessAction, ProcessIdentity, QueryTarget, Resolution,
};

use crate::backend::{InvestigationTarget, WorkspaceBackend, WorkspaceSnapshot};
use crate::processes::ProcessActionFlow;
use crate::session::WorkspaceId;
use crate::state::DataState;
use crate::workspaces::{LoadPresentation, interactions_enabled};

/// macOS 文件能力的 best-effort 受限原因（与 platform 稳定原因键同形）。
const MACOS_FILES_PARTIAL_REASON: &str = "macOS 无 /proc/locks：真实锁经 lsof 锁标志位 best-effort";
/// Windows 文件锁能力的稳定原因键占位。
const WINDOWS_FILE_LOCKS_REASON: &str = "Windows 平台不提供文件锁枚举（parity §10）";
/// Windows 进程控制能力的稳定原因键占位。
const WINDOWS_PROCESS_CONTROL_REASON: &str = "Windows 平台不支持进程控制操作（parity §10）";

fn pid(value: u32) -> Pid {
    Pid::new(value).unwrap_or(Pid::MIN)
}

fn action_identity() -> ProcessIdentity {
    ProcessIdentity::new(pid(4242), Some(SystemTime::UNIX_EPOCH), None)
}

fn file_entry() -> FileInventoryEntry {
    FileInventoryEntry {
        pid: pid(4242),
        process: String::from("synthetic"),
        path: PathBuf::from("/tmp/runquiry-contract"),
        fd: Some(3),
        lock: None,
    }
}

/// 一次快照经生产 `LoadPresentation::apply` 后的语义结论。
struct LoadContract {
    state: DataState,
    partial_note: Option<Arc<str>>,
    boundary_reason: Option<Arc<str>>,
    is_partial: bool,
}

impl LoadContract {
    /// 平台无关契约：能力四态必须落到对应状态，边界原因不得丢失。
    fn assert_matches_capability(&self, capability: &CapabilityStatus) {
        match capability {
            CapabilityStatus::Supported | CapabilityStatus::Partial(_) => {
                assert!(
                    matches!(self.state, DataState::Ready | DataState::Empty),
                    "{capability:?} 不得映射为边界或错误状态，实际 {:?}",
                    self.state
                );
            }
            CapabilityStatus::Unsupported(_) => {
                assert_eq!(self.state, DataState::Unsupported);
                assert!(
                    self.boundary_reason.is_some(),
                    "Unsupported 原因必须保留给呈现层"
                );
                assert!(
                    !interactions_enabled(self.state),
                    "Unsupported 工作区必须禁用模式/筛选交互"
                );
            }
            CapabilityStatus::Unavailable(_) => {
                assert_eq!(self.state, DataState::Unavailable);
                assert!(
                    self.boundary_reason.is_some(),
                    "Unavailable 原因必须保留给呈现层"
                );
                assert!(
                    !interactions_enabled(self.state),
                    "Unavailable 工作区必须禁用模式/筛选交互"
                );
            }
        }
    }
}

/// 对一次采集结果执行生产状态转换并提取契约结论。
fn apply_contract<T>(
    capability: &CapabilityStatus,
    inspection: &Inspection<Arc<[T]>>,
) -> LoadContract {
    let mut load = LoadPresentation::default();
    assert!(load.apply(Generation::first(), capability, inspection.clone()));
    LoadContract {
        state: load.state,
        partial_note: load.capability_note.clone(),
        boundary_reason: load.boundary_reason.clone(),
        is_partial: load.is_partial(),
    }
}

/// 快照 →（能力，契约结论）；与产品壳层 `ShellData::apply` 的路由一致。
fn contract_of(snapshot: &WorkspaceSnapshot) -> (CapabilityStatus, LoadContract) {
    match snapshot {
        WorkspaceSnapshot::Processes {
            capability,
            inspection,
        } => (capability.clone(), apply_contract(capability, inspection)),
        WorkspaceSnapshot::Ports {
            capability,
            inspection,
        } => (capability.clone(), apply_contract(capability, inspection)),
        WorkspaceSnapshot::Containers {
            capability,
            inspection,
        } => (capability.clone(), apply_contract(capability, inspection)),
        WorkspaceSnapshot::FileLocks {
            capability,
            inspection,
        } => (capability.clone(), apply_contract(capability, inspection)),
    }
}

/// 对后端全部四个工作区执行同一套契约：路由正确、状态语义与控制门控一致。
fn run_shared_contract(backend: &dyn WorkspaceBackend) {
    let control = backend.process_control_capability();
    for workspace in WorkspaceId::ALL {
        let snapshot = backend.load(workspace);
        assert_eq!(snapshot.workspace(), workspace, "快照不得串页");
        let (capability, contract) = contract_of(&snapshot);
        contract.assert_matches_capability(&capability);

        // 控制能力门控与能力态一致；被拒绝的请求不得留下错误记录。
        let mut flow = ProcessActionFlow::new();
        let accepted = flow.request(&control, action_identity(), ProcessAction::Terminate);
        assert_eq!(accepted, control.is_usable(), "{workspace:?} 门控不一致");
        if !accepted {
            assert!(flow.last_error().is_none(), "能力拒绝不得伪装成错误路径");
        }
    }
}

/// 四工作区共用的快照行；契约关注能力语义，行集合给最小真实形状。
fn base_snapshot(workspace: WorkspaceId) -> WorkspaceSnapshot {
    match workspace {
        WorkspaceId::Processes => WorkspaceSnapshot::Processes {
            capability: CapabilityStatus::Supported,
            inspection: Inspection::complete(Arc::default()),
        },
        WorkspaceId::Ports => WorkspaceSnapshot::Ports {
            capability: CapabilityStatus::Supported,
            inspection: Inspection::complete(Arc::default()),
        },
        WorkspaceId::Containers => WorkspaceSnapshot::Containers {
            capability: CapabilityStatus::Supported,
            inspection: Inspection::complete(Arc::default()),
        },
        WorkspaceId::FileLocks => WorkspaceSnapshot::FileLocks {
            capability: CapabilityStatus::Supported,
            inspection: Inspection::complete(Arc::default()),
        },
    }
}

/// Linux 风格：文件能力与进程控制可用。
struct LinuxStyleBackend;

impl WorkspaceBackend for LinuxStyleBackend {
    fn load(&self, workspace: WorkspaceId) -> WorkspaceSnapshot {
        let mut snapshot = base_snapshot(workspace);
        if let WorkspaceSnapshot::FileLocks { inspection, .. } = &mut snapshot {
            *inspection = Inspection::complete(Arc::from([file_entry()]));
        }
        snapshot
    }

    fn resolve(&self, _: &QueryTarget) -> Result<Resolution<InvestigationTarget>, InspectError> {
        Err(InspectError::Unsupported {
            reason: String::from("契约测试不使用解析路径"),
        })
    }

    fn analyze(&self, _: &ProcessIdentity) -> Result<Inspection<Analysis>, InspectError> {
        Err(InspectError::Unsupported {
            reason: String::from("契约测试不使用分析路径"),
        })
    }

    fn process_control_capability(&self) -> CapabilityStatus {
        CapabilityStatus::Supported
    }
}

/// macOS 风格：文件能力 best-effort/Partial（数据与诊断保留），控制可用。
struct MacosStyleBackend;

impl WorkspaceBackend for MacosStyleBackend {
    fn load(&self, workspace: WorkspaceId) -> WorkspaceSnapshot {
        let mut snapshot = base_snapshot(workspace);
        if let WorkspaceSnapshot::FileLocks {
            capability,
            inspection,
        } = &mut snapshot
        {
            *capability = CapabilityStatus::Partial(String::from(MACOS_FILES_PARTIAL_REASON));
            *inspection = Inspection::partial(
                Arc::from([file_entry()]),
                vec![DiagnosticIssue::new(
                    DiagnosticCode::ExternalToolFailed,
                    String::from("lsof 部分条目解析失败"),
                )],
            );
        }
        snapshot
    }

    fn resolve(&self, _: &QueryTarget) -> Result<Resolution<InvestigationTarget>, InspectError> {
        Err(InspectError::Unsupported {
            reason: String::from("契约测试不使用解析路径"),
        })
    }

    fn analyze(&self, _: &ProcessIdentity) -> Result<Inspection<Analysis>, InspectError> {
        Err(InspectError::Unsupported {
            reason: String::from("契约测试不使用分析路径"),
        })
    }

    fn process_control_capability(&self) -> CapabilityStatus {
        CapabilityStatus::Supported
    }
}

/// Windows 风格：文件锁与进程控制 Unsupported（原因键稳定），其余工作区可用。
struct WindowsStyleBackend;

impl WorkspaceBackend for WindowsStyleBackend {
    fn load(&self, workspace: WorkspaceId) -> WorkspaceSnapshot {
        let mut snapshot = base_snapshot(workspace);
        if let WorkspaceSnapshot::FileLocks {
            capability,
            inspection,
        } = &mut snapshot
        {
            *capability = CapabilityStatus::Unsupported(String::from(WINDOWS_FILE_LOCKS_REASON));
            *inspection = Inspection::failed(vec![DiagnosticIssue::new(
                DiagnosticCode::Unsupported,
                String::from(WINDOWS_FILE_LOCKS_REASON),
            )]);
        }
        snapshot
    }

    fn resolve(&self, _: &QueryTarget) -> Result<Resolution<InvestigationTarget>, InspectError> {
        Err(InspectError::Unsupported {
            reason: String::from(WINDOWS_FILE_LOCKS_REASON),
        })
    }

    fn analyze(&self, _: &ProcessIdentity) -> Result<Inspection<Analysis>, InspectError> {
        Err(InspectError::Unsupported {
            reason: String::from(WINDOWS_FILE_LOCKS_REASON),
        })
    }

    fn process_control_capability(&self) -> CapabilityStatus {
        CapabilityStatus::Unsupported(String::from(WINDOWS_PROCESS_CONTROL_REASON))
    }
}

/// 环境不可用风格：平台构造失败时的诚实边界（UnavailableBackend 同形）。
struct UnavailableStyleBackend {
    reason: String,
}

impl WorkspaceBackend for UnavailableStyleBackend {
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
        CapabilityStatus::Unavailable(self.reason.clone())
    }
}

/// 快照路由的文件工作区行数；`None` 表示快照串页或行缺失。
fn file_locks_row_count(snapshot: &WorkspaceSnapshot) -> Option<usize> {
    match snapshot {
        WorkspaceSnapshot::FileLocks { inspection, .. } => {
            inspection.data.as_ref().map(|rows| rows.len())
        }
        _ => None,
    }
}

#[test]
fn linux_style_serves_files_and_process_control_with_real_empty_collections() {
    let backend = LinuxStyleBackend;
    run_shared_contract(&backend);

    for workspace in WorkspaceId::ALL {
        let snapshot = backend.load(workspace);
        let (_, contract) = contract_of(&snapshot);
        if workspace == WorkspaceId::FileLocks {
            assert_eq!(contract.state, DataState::Ready, "文件行应保留");
            assert!(!contract.is_partial);
        } else {
            assert_eq!(
                contract.state,
                DataState::Empty,
                "Supported + 空快照必须是空集合而非错误或边界"
            );
            assert!(contract.partial_note.is_none());
        }
    }

    // 控制可用：请求→确认→后台结果走完整生产状态机。
    let mut flow = ProcessActionFlow::new();
    assert!(flow.request(
        &backend.process_control_capability(),
        action_identity(),
        ProcessAction::Terminate,
    ));
    let request = flow.confirm();
    assert!(request.is_some(), "Supported 能力下确认必须产生后台请求");
    let Some(request) = request else { return };
    let completion = flow.complete(&request, Ok(()));
    assert!(completion.is_some(), "同代际成功结果必须被接受");
}

#[test]
fn macos_style_files_are_partial_with_retained_data_and_issues() {
    let backend = MacosStyleBackend;
    run_shared_contract(&backend);

    let snapshot = backend.load(WorkspaceId::FileLocks);
    let (capability, contract) = contract_of(&snapshot);
    assert!(matches!(capability, CapabilityStatus::Partial(_)));
    assert_eq!(contract.state, DataState::Ready, "Partial 数据必须保留");
    assert!(contract.is_partial, "Partial + 诊断必须呈现受限横幅");
    assert_eq!(
        contract.partial_note.as_deref(),
        Some(MACOS_FILES_PARTIAL_REASON)
    );
    assert!(contract.boundary_reason.is_none());
    assert_eq!(file_locks_row_count(&snapshot), Some(1));
}

#[test]
fn windows_style_boundaries_are_not_errors_and_block_actions() {
    let backend = WindowsStyleBackend;
    run_shared_contract(&backend);

    let snapshot = backend.load(WorkspaceId::FileLocks);
    let (capability, contract) = contract_of(&snapshot);
    assert!(matches!(capability, CapabilityStatus::Unsupported(_)));
    assert_eq!(contract.state, DataState::Unsupported);
    assert_eq!(
        contract.boundary_reason.as_deref(),
        Some(WINDOWS_FILE_LOCKS_REASON)
    );
    // 数据缺失 + 能力边界：不得伪装成空集合、加载失败或错误。
    assert_eq!(file_locks_row_count(&snapshot), None);

    // 控制不可用：请求被拒且不进入错误路径；确认流程无可执行动作。
    let mut flow = ProcessActionFlow::new();
    assert!(!flow.request(
        &backend.process_control_capability(),
        action_identity(),
        ProcessAction::Terminate,
    ));
    assert!(!flow.is_confirming());
    assert!(flow.last_error().is_none());
}

#[test]
fn unavailable_style_is_a_boundary_with_reason_not_an_error() {
    let backend = UnavailableStyleBackend {
        reason: String::from("平台采集器不可用：no collector"),
    };
    run_shared_contract(&backend);
    for workspace in WorkspaceId::ALL {
        let snapshot = backend.load(workspace);
        let (capability, contract) = contract_of(&snapshot);
        assert_eq!(contract.state, DataState::Unavailable);
        assert!(contract.boundary_reason.is_some());
        assert!(!interactions_enabled(contract.state));
        assert_eq!(capability.reason(), contract.boundary_reason.as_deref());
    }
}

#[test]
fn permission_tool_failure_and_real_empty_are_not_conflated() {
    let supported = CapabilityStatus::Supported;

    let mut load = LoadPresentation::default();
    assert!(load.apply(
        Generation::first(),
        &supported,
        Inspection::<Arc<[u8]>>::failed(vec![DiagnosticIssue::new(
            DiagnosticCode::PermissionDenied,
            String::from("permission"),
        )]),
    ));
    assert_eq!(load.state, DataState::PermissionDenied);

    let mut load = LoadPresentation::default();
    assert!(load.apply(
        Generation::first(),
        &supported,
        Inspection::<Arc<[u8]>>::failed(vec![DiagnosticIssue::new(
            DiagnosticCode::ExternalToolFailed,
            String::from("lsof missing"),
        )]),
    ));
    assert_eq!(load.state, DataState::Error, "工具完全失败是错误而非边界");

    let mut load = LoadPresentation::default();
    assert!(load.apply(
        Generation::first(),
        &supported,
        Inspection::<Arc<[u8]>>::complete(Arc::default()),
    ));
    assert_eq!(load.state, DataState::Empty);
}

#[test]
fn capability_change_revokes_pending_confirmation() {
    let mut flow = ProcessActionFlow::new();
    assert!(flow.request(
        &CapabilityStatus::Supported,
        action_identity(),
        ProcessAction::Terminate,
    ));
    assert!(flow.is_confirming());

    // 能力退化为 Unsupported：确认被撤销，confirm 不得再产生后台请求。
    let revoked = flow.revoke_confirmation_if_unusable(&CapabilityStatus::Unsupported(
        String::from(WINDOWS_PROCESS_CONTROL_REASON),
    ));
    assert!(revoked);
    assert!(!flow.is_confirming());
    assert!(flow.confirm().is_none());

    // Partial 仍可用：不撤销，确认可继续。
    let mut flow = ProcessActionFlow::new();
    assert!(flow.request(
        &CapabilityStatus::Partial(String::from("limited")),
        action_identity(),
        ProcessAction::Terminate,
    ));
    assert!(
        !flow.revoke_confirmation_if_unusable(&CapabilityStatus::Partial(String::from("limited")))
    );
    assert!(flow.confirm().is_some());
}

#[test]
fn unavailable_control_blocks_request_without_error_path() {
    let mut flow = ProcessActionFlow::new();
    let unavailable = CapabilityStatus::Unavailable(String::from("collector unavailable"));
    assert!(!flow.request(&unavailable, action_identity(), ProcessAction::Kill));
    assert!(!flow.is_confirming());
    assert!(flow.last_error().is_none());
}
