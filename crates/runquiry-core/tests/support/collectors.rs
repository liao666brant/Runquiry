//! B1 分析管线假采集器：core 六端口 + 来源证据 + 健康检查探针的可配置假实现。
//!
//! 全部为确定性合成数据：不读真实进程、不依赖时钟与宿主机状态；
//! 失败注入通过显式字段（`fail_*` / `outcome`）表达。

// 测试 crate 非 lib 目标，support 模块不对外导出：unreachable_pub 不适用。
#![allow(unreachable_pub)]
#![allow(dead_code)]

use std::time::{Duration, SystemTime};

use runquiry_core::{
    CapabilityStatus, ContainerHealthcheckProbe, ContainerInventory, ContainerKey,
    ContainerSummary, DiagnosticCode, DiagnosticIssue, FileLockEntry, HealthStatus,
    HealthcheckStatus, InspectError, Inspection, Pid, ProcessDetails, ProcessDetailsProvider,
    ProcessIdentity, ProcessInventory, ProcessSummary, SocketEntry, SourceEvidence,
    SourceEvidenceProvider,
};

/// 与 fixture 封套一致的确定性采集时刻（epoch 毫秒 `1700000000000`）。
#[must_use]
pub fn captured_at() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000)
}

/// 构造合成进程摘要（字段默认值与 fixture 约定对齐）。
#[must_use]
pub fn summary(
    pid: u32,
    parent: Option<u32>,
    command: &str,
    command_line: Option<&str>,
) -> ProcessSummary {
    let identity =
        ProcessIdentity::new(Pid::new(pid).unwrap_or(Pid::MIN), Some(captured_at()), None);
    ProcessSummary {
        identity,
        parent_pid: parent.and_then(|p| Pid::new(p).ok()),
        command: String::from(command),
        command_line: command_line.map(String::from),
        user: None,
        health: HealthStatus::Unknown,
        container: None,
        exe_deleted: false,
        capabilities: Vec::new(),
    }
}

/// 进程清单假实现：可注入清单失败。
#[derive(Debug)]
pub struct FakeInventory {
    pub capability: CapabilityStatus,
    pub entries: Vec<ProcessSummary>,
    pub fail: bool,
}

impl FakeInventory {
    #[must_use]
    pub const fn new(entries: Vec<ProcessSummary>) -> Self {
        Self {
            capability: CapabilityStatus::Supported,
            entries,
            fail: false,
        }
    }
}

impl ProcessInventory for FakeInventory {
    fn capability(&self) -> CapabilityStatus {
        self.capability.clone()
    }

    fn list(&self) -> Inspection<Vec<ProcessSummary>> {
        if self.fail {
            Inspection::failed(vec![DiagnosticIssue::new(
                DiagnosticCode::Unknown,
                String::from("合成场景：清单采集失败"),
            )])
        } else {
            Inspection::complete(self.entries.clone())
        }
    }
}

/// 详情提供者假实现：按场景返回详情或注入错误。
#[derive(Debug)]
pub struct FakeDetails {
    pub baseline: ProcessIdentity,
    pub outcome: Result<Inspection<ProcessDetails>, InspectError>,
}

impl FakeDetails {
    #[must_use]
    pub fn ok(baseline: &ProcessIdentity, details: ProcessDetails) -> Self {
        Self {
            baseline: baseline.clone(),
            outcome: Ok(Inspection::complete(details)),
        }
    }

    #[must_use]
    pub fn err(baseline: &ProcessIdentity, error: InspectError) -> Self {
        Self {
            baseline: baseline.clone(),
            outcome: Err(error),
        }
    }

    #[must_use]
    pub fn partial(
        baseline: &ProcessIdentity,
        details: ProcessDetails,
        issues: Vec<DiagnosticIssue>,
    ) -> Self {
        Self {
            baseline: baseline.clone(),
            outcome: Ok(Inspection::partial(details, issues)),
        }
    }
}

impl ProcessDetailsProvider for FakeDetails {
    fn capability(&self) -> CapabilityStatus {
        CapabilityStatus::Supported
    }

    fn details(
        &self,
        identity: &ProcessIdentity,
    ) -> Result<Inspection<ProcessDetails>, InspectError> {
        // 契约：expected 与平台重读的 current 不一致即拒绝（PID 复用防护）。
        if !identity.same_process(&self.baseline) {
            return Err(InspectError::ProcessChanged {
                identity: self.baseline.clone(),
            });
        }
        self.outcome.clone()
    }
}

/// 网络采集假实现：直接返回预置的 [`Inspection`]。
#[derive(Debug)]
pub struct FakeNetwork {
    pub result: Inspection<Vec<SocketEntry>>,
}

impl runquiry_core::NetworkInventory for FakeNetwork {
    fn capability(&self) -> CapabilityStatus {
        CapabilityStatus::Supported
    }

    fn open_ports(&self) -> Inspection<Vec<runquiry_core::OpenPortEntry>> {
        Inspection::complete(Vec::new())
    }

    fn sockets_of(&self, _pid: Pid) -> Inspection<Vec<SocketEntry>> {
        self.result.clone()
    }
}

/// 容器清单假实现：仅提供 list 数据，`host_pid` 恒 `None`。
#[derive(Debug)]
pub struct FakeContainers {
    pub containers: Vec<ContainerSummary>,
}

impl ContainerInventory for FakeContainers {
    fn capability(&self) -> CapabilityStatus {
        CapabilityStatus::Supported
    }

    fn list(&self) -> Inspection<Vec<ContainerSummary>> {
        Inspection::complete(self.containers.clone())
    }

    fn host_pid(&self, _key: &ContainerKey) -> Result<Option<Pid>, InspectError> {
        Ok(None)
    }
}

/// 进程级文件锁假实现（additive `ProcessFileLocks` 端口）。
#[derive(Debug)]
pub struct FakeProcessLocks {
    pub locks: Vec<FileLockEntry>,
}

impl runquiry_core::ProcessFileLocks for FakeProcessLocks {
    fn locks_of(&self, _pid: Pid) -> Inspection<Vec<FileLockEntry>> {
        Inspection::complete(self.locks.clone())
    }
}

/// 来源证据假实现。
#[derive(Debug)]
pub struct FakeEvidence {
    pub evidence: SourceEvidence,
}

impl SourceEvidenceProvider for FakeEvidence {
    fn evidence(&self, _ancestry: &[ProcessSummary]) -> SourceEvidence {
        self.evidence.clone()
    }
}

/// 容器健康检查探针假实现：恒返回预置状态。
#[derive(Debug)]
pub struct FakeHealthcheck {
    pub status: Option<HealthcheckStatus>,
}

impl ContainerHealthcheckProbe for FakeHealthcheck {
    fn healthcheck_status(&self, _id: &str, _runtime: &str) -> Option<HealthcheckStatus> {
        self.status
    }
}
