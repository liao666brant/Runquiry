//! 七个平台端口的假后端：按 [`Scenario`] 注入失败，构成失败注入入口。
//!
//! 契约语义与真实实现一致：
//! * 采集走 [`Inspection`] 部分成功语义，快照时刻来自确定性时钟；
//! * `ProcessController` 按契约在动作前比对 expected / current 身份，
//!   不一致（含 current `start_time` 为 `None`）返回
//!   [`InspectError::ProcessChanged`] 且实际动作计数保持 0；
//! * `CommandRunner` 按场景返回成功输出、工具缺失、超时或不可解析输出。

// 测试 crate 非 lib 目标，support 模块不对外导出：unreachable_pub 不适用；
// 共享模块由各测试目标按需取用，未用项不构成告警。
#![allow(unreachable_pub)]
#![allow(dead_code)]

use std::cell::RefCell;
use std::time::{Duration, SystemTime};

use runquiry_core::{
    CapabilityStatus, CommandOutput, CommandRunner, CommandSpec, ContainerInventory, ContainerKey,
    ContainerSummary, DiagnosticCode, DiagnosticIssue, FileInventory, FileLockEntry, InspectError,
    Inspection, LockMode, LockType, NetworkInventory, OpenPortEntry, Pid, ProcessAction,
    ProcessController, ProcessDetails, ProcessDetailsProvider, ProcessIdentity, ProcessInventory,
    ProcessSummary, Protocol, SocketEntry,
};

use super::{CAPTURED_AT_MS, FXT_PID, Generation, Scenario};

/// 合成采集时刻。
fn captured_at() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_millis(CAPTURED_AT_MS)
}

fn issue(code: DiagnosticCode, message: &str) -> DiagnosticIssue {
    DiagnosticIssue::new(code, String::from(message))
}

/// 七端口假平台后端：单一结构按场景注入数据与失败。
#[derive(Debug)]
pub struct FakePlatform {
    scenario: Scenario,
    /// 固定请求代数。
    generation: Generation,
    /// 快照时看到的基线身份（详情提供者据此判定 `NotFound`）。
    baseline: ProcessIdentity,
    /// 进程控制在动作前重读得到的 current 身份（PID 复用注入点）。
    current: ProcessIdentity,
    /// 实际执行过的动作（断言副作用计数）。
    executed: RefCell<Vec<(Pid, ProcessAction)>>,
}

impl FakePlatform {
    /// 以场景构造；基线与 current 身份一致（同 PID、同合成 `start_time`）。
    ///
    /// # Errors
    /// 合成常量构造失败时返回错误（正常常量下不会发生）。
    pub fn new(scenario: Scenario) -> Result<Self, String> {
        let baseline = synthetic_identity(
            Pid::new(FXT_PID).map_err(|e| e.to_string())?,
            Some(captured_at()),
        );
        let current = baseline.clone();
        Ok(Self {
            scenario,
            generation: Generation::FIXTURE,
            baseline,
            current,
            executed: RefCell::new(Vec::new()),
        })
    }

    /// 注入 PID 复用：基线保持不变，把平台重读到的 current 改为给定身份。
    #[must_use]
    pub fn with_current(mut self, current: ProcessIdentity) -> Self {
        self.current = current;
        self
    }

    /// 固定 generation（fixture 同值约定）。
    pub const fn generation(&self) -> u64 {
        self.generation.get()
    }

    /// 实际执行的动作数量。
    pub fn executed_count(&self) -> usize {
        self.executed.borrow().len()
    }

    /// 按场景把数据包装为 [`Inspection`]（失败注入入口）。
    fn inspect<T>(&self, data: Vec<T>) -> Inspection<Vec<T>> {
        let at = captured_at();
        match self.scenario {
            Scenario::Normal | Scenario::MalformedOutput => {
                Inspection::with_captured_at(Some(data), Vec::new(), at)
            }
            Scenario::Empty => Inspection::with_captured_at(Some(Vec::new()), Vec::new(), at),
            Scenario::Partial => Inspection::with_captured_at(
                Some(data),
                vec![issue(
                    DiagnosticCode::PermissionDenied,
                    "部分条目读取受限（合成场景）",
                )],
                at,
            ),
            Scenario::PermissionDenied => Inspection::with_captured_at(
                None,
                vec![issue(
                    DiagnosticCode::PermissionDenied,
                    "采集整体被拒绝（合成场景）",
                )],
                at,
            ),
            Scenario::ToolMissing => Inspection::with_captured_at(
                None,
                vec![issue(
                    DiagnosticCode::ExternalToolFailed,
                    "fxt CLI 缺失（合成场景）",
                )],
                at,
            ),
            Scenario::Timeout => Inspection::with_captured_at(
                None,
                vec![issue(DiagnosticCode::Timeout, "采集超时（合成场景）")],
                at,
            ),
        }
    }

    /// 按场景给出平台能力状态。
    fn capability_for(&self) -> CapabilityStatus {
        match self.scenario {
            Scenario::ToolMissing => {
                CapabilityStatus::Unavailable(String::from("容器运行时 CLI 未安装（合成场景）"))
            }
            _ => CapabilityStatus::Supported,
        }
    }
}

fn synthetic_identity(pid: Pid, start_time: Option<SystemTime>) -> ProcessIdentity {
    ProcessIdentity::new(
        pid,
        start_time,
        Some(std::path::PathBuf::from(
            "/opt/runquiry-fixtures/bin/fxt-daemon",
        )),
    )
}

impl ProcessInventory for FakePlatform {
    fn capability(&self) -> CapabilityStatus {
        self.capability_for()
    }

    fn list(&self) -> Inspection<Vec<ProcessSummary>> {
        let summary = |pid: Pid| ProcessSummary {
            identity: synthetic_identity(pid, Some(captured_at())),
            parent_pid: None,
            command: String::from("fxt-daemon"),
            command_line: Some(String::from(
                "fxt-daemon --config /opt/runquiry-fixtures/etc/fxt.conf",
            )),
            user: Some(String::from("fixture-user")),
        };
        let pid = self.baseline.pid();
        self.inspect(vec![summary(pid)])
    }
}

impl ProcessDetailsProvider for FakePlatform {
    fn capability(&self) -> CapabilityStatus {
        self.capability_for()
    }

    fn details(&self, identity: &ProcessIdentity) -> Result<ProcessDetails, InspectError> {
        if !identity.same_process(&self.baseline) {
            return Err(InspectError::NotFound {
                subject: format!("PID {}", identity.pid()),
            });
        }
        if self.scenario == Scenario::PermissionDenied {
            return Err(InspectError::PermissionDenied {
                subject: String::from("进程详情（合成场景）"),
            });
        }
        Ok(ProcessDetails {
            identity: self.baseline.clone(),
            cpu_percent: Some(12.5),
            memory_rss_bytes: Some(20_480),
            memory_percent: Some(0.5),
            working_dir: Some(std::path::PathBuf::from("/opt/runquiry-fixtures/var")),
            environment: Vec::new(),
            children: Vec::new(),
        })
    }
}

impl NetworkInventory for FakePlatform {
    fn capability(&self) -> CapabilityStatus {
        self.capability_for()
    }

    fn open_ports(&self) -> Inspection<Vec<OpenPortEntry>> {
        // 合成端口恒在 1..=65535，unwrap_or 仅满足类型检查，不构成失败路径。
        let port = runquiry_core::Port::new(8443).unwrap_or(runquiry_core::Port::MIN);
        let entry = OpenPortEntry {
            pid: Some(self.baseline.pid()),
            port,
            address: String::from("0.0.0.0"),
            protocol: Protocol::Tcp,
            state: String::from("LISTEN"),
        };
        self.inspect(vec![entry])
    }

    fn sockets_of(&self, pid: Pid) -> Inspection<Vec<SocketEntry>> {
        let port = runquiry_core::Port::new(8443).unwrap_or(runquiry_core::Port::MIN);
        let entry = SocketEntry {
            inode: None,
            port: Some(port),
            address: String::from("0.0.0.0"),
            remote_addr: None,
            state: String::from("LISTEN"),
            protocol: Protocol::Tcp,
            owner_pid: Some(pid),
        };
        self.inspect(vec![entry])
    }
}

impl ContainerInventory for FakePlatform {
    fn capability(&self) -> CapabilityStatus {
        self.capability_for()
    }

    fn list(&self) -> Inspection<Vec<ContainerSummary>> {
        let container = ContainerSummary {
            key: ContainerKey {
                runtime: String::from("docker"),
                id: String::from("fxt0001container"),
            },
            name: Some(String::from("fxt-web")),
            image: Some(String::from("registry.example.internal/fxt-web:1.0")),
            status: Some(String::from("Up 2 hours")),
            health: None,
            host_pid: Some(self.baseline.pid()),
            started_at: Some(captured_at()),
        };
        // 工具缺失语义与 fixture 一致：docker 可用 + podman 缺失 = 部分成功。
        if self.scenario == Scenario::ToolMissing {
            let at = captured_at();
            return Inspection::with_captured_at(
                Some(vec![container]),
                vec![issue(
                    DiagnosticCode::ExternalToolFailed,
                    "运行时 podman CLI 缺失，其容器未计入（合成场景）",
                )],
                at,
            );
        }
        self.inspect(vec![container])
    }

    fn host_pid(&self, key: &ContainerKey) -> Result<Option<Pid>, InspectError> {
        if key.id == "fxt0001container" {
            Ok(Some(self.baseline.pid()))
        } else {
            Ok(None)
        }
    }
}

impl FileInventory for FakePlatform {
    fn capability(&self) -> CapabilityStatus {
        self.capability_for()
    }

    fn holders(&self, path: &std::path::Path) -> Inspection<Vec<FileLockEntry>> {
        let entry = FileLockEntry {
            pid: self.baseline.pid(),
            process: String::from("fxt-daemon"),
            path: path.to_path_buf(),
            lock_type: LockType::Flock,
            mode: LockMode::Write,
        };
        self.inspect(vec![entry])
    }
}

impl ProcessController for FakePlatform {
    fn capability(&self) -> CapabilityStatus {
        self.capability_for()
    }

    fn execute(
        &self,
        identity: &ProcessIdentity,
        action: ProcessAction,
    ) -> Result<(), InspectError> {
        // 契约：expected（确认流程快照）与 current（平台重读）不一致即拒绝，
        // 不产生副作用；ProcessChanged 携带重读得到的 current 身份。
        if !identity.same_process(&self.current) {
            return Err(InspectError::ProcessChanged {
                identity: self.current.clone(),
            });
        }
        self.executed
            .borrow_mut()
            .push((self.current.pid(), action));
        Ok(())
    }
}

impl CommandRunner for FakePlatform {
    fn run(&self, spec: &CommandSpec, _timeout: Duration) -> Result<CommandOutput, InspectError> {
        let program = spec.program.clone();
        match self.scenario {
            Scenario::ToolMissing => Err(InspectError::ExternalTool {
                program,
                detail: String::from("程序未安装（合成场景）"),
            }),
            Scenario::Timeout => Err(InspectError::ExternalTool {
                program,
                detail: String::from("超时（合成场景）"),
            }),
            Scenario::MalformedOutput => Ok(CommandOutput {
                exit_code: Some(0),
                stdout: vec![0xFF, 0xFE, 0x00],
                stderr: Vec::new(),
                stdout_truncated: false,
                stderr_truncated: false,
            }),
            _ => Ok(CommandOutput {
                exit_code: Some(0),
                stdout: b"[]".to_vec(),
                stderr: Vec::new(),
                stdout_truncated: false,
                stderr_truncated: false,
            }),
        }
    }
}
