//! 分析管线（parity §4：witr `internal/pipeline/analyze.go` 的 `AnalyzePID`）。
//!
//! 单一入口，按 witr 固定顺序组合：祖先链 → 来源识别（经
//! `SourceEvidenceProvider`）→ 目标进程选取 → 容器健康检查补全 → 子进程与
//! 扩展信息收集 → Socket / 文件锁 → 告警生成 → 组装结果。单采集器失败只
//! 追加诊断，不抹掉已有数据；子进程快照失败静默降级为空。

use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::ancestry::resolve_ancestry;
use crate::model::diagnostic::{DiagnosticCode, DiagnosticIssue};
use crate::model::error::InspectError;
use crate::model::ids::Pid;
use crate::model::inspection::Inspection;
use crate::model::process::{ProcessDetails, ProcessIdentity, ProcessSummary};
use crate::model::source::SourceType;
use crate::port::container::{ContainerHealthcheckProbe, ContainerInventory};
use crate::port::file::{FileLockEntry, ProcessFileLocks};
use crate::port::network::{NetworkInventory, SocketEntry};
use crate::port::process::{ProcessDetailsProvider, ProcessInventory};
use crate::port::source::{SourceEvidence, SourceEvidenceProvider};
use crate::source_detect::detect_source;

/// 管线采集端口集合（全部同步；读取由注入的实现完成，core 不发起系统调用）。
pub struct AnalysisPorts<'a> {
    /// 进程清单（祖先链读取与子进程快照来源）。
    pub inventory: &'a dyn ProcessInventory,
    /// 进程详情（扩展信息、环境变量与工作目录）。
    pub details: &'a dyn ProcessDetailsProvider,
    /// 网络（目标进程 Socket）。
    pub network: &'a dyn NetworkInventory,
    /// 容器清单（容器能力承载；健康检查探测见 `healthcheck`）。
    pub containers: &'a dyn ContainerInventory,
    /// 来源证据（cgroup / 环境变量 / systemd 状态）。
    pub evidence: &'a dyn SourceEvidenceProvider,
    /// 容器健康检查定义探针（additive；未提供时健康检查保持 `None`）。
    pub healthcheck: Option<&'a dyn ContainerHealthcheckProbe>,
    /// 进程级文件锁查询（additive；未提供时文件锁降级为空）。
    pub process_locks: Option<&'a dyn ProcessFileLocks>,
}

impl std::fmt::Debug for AnalysisPorts<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 端口为 trait 对象，无统一 Debug 面；仅暴露注入情况。
        f.debug_struct("AnalysisPorts")
            .field("healthcheck_provided", &self.healthcheck.is_some())
            .field("process_locks_provided", &self.process_locks.is_some())
            .finish_non_exhaustive()
    }
}

/// 分析结果（parity：`model.Result` 的 Runquiry 对应物，确定可序列化）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Analysis {
    /// 链尾（目标）进程摘要。
    pub target: ProcessSummary,
    /// 祖先链（root→target；单跳失败时截断）。
    pub ancestry: Vec<ProcessSummary>,
    /// 解析后的目标描述（witr `Result.ResolvedTarget`：链尾 Command，空回退
    /// "unknown"）。
    pub resolved_target: String,
    /// 启动来源（永不返回空，兜底 unknown）。
    pub source: crate::model::source::Source,
    /// 服务重启计数（仅 systemd 来源从 `Source.details` 的 `NRestarts` 解析）。
    pub restart_count: u64,
    /// 子进程 PID（按 PID 升序；快照失败静默为空）。
    pub children: Vec<Pid>,
    /// 目标进程 Socket。
    pub sockets: Vec<SocketEntry>,
    /// 目标进程文件锁（`ProcessFileLocks` 未提供时为空）。
    pub file_locks: Vec<FileLockEntry>,
    /// 进程详情（读取失败时为 `None`，资源详情降级）。
    pub details: Option<ProcessDetails>,
    /// 告警列表（顺序与 witr `Warnings` 逐条一致，见 [`crate::warnings`]）。
    pub warnings: Vec<crate::warnings::Warning>,
}

/// 分析入口：按 witr `AnalyzePID` 的组合顺序执行。
///
/// # Errors
/// 祖先链为空（目标已退出）→ [`InspectError::NotFound`]；详情读取返回
/// `ProcessChanged`（PID 复用，身份经 `same_process` 判定不一致）或
/// `NotFound`（读取间隙退出）时整体失败；其余单采集器错误降级为诊断。
pub fn analyze(
    target: &ProcessIdentity,
    ports: &AnalysisPorts<'_>,
    now: SystemTime,
    is_windows: bool,
) -> Result<Inspection<Analysis>, InspectError> {
    // 0. 进程清单快照：一次采集，祖先链与子进程快照共用同一份
    //    （避免逐跳重复全量扫描，也保证祖先链来自同一时刻的一致快照）。
    let snapshot = ports.inventory.list();
    let snapshot_entries = snapshot.data.clone().unwrap_or_default();

    // 1. 祖先链：单跳失败截断（失败原因经 reader 错误通道回传为诊断）；
    //    目标不可读 → NotFound。
    let reader = |pid: Pid| -> Result<Option<ProcessSummary>, DiagnosticIssue> {
        snapshot_entries
            .iter()
            .find(|p| p.identity.pid() == pid)
            .cloned()
            .map(Some)
            .ok_or_else(|| {
                DiagnosticIssue::new(
                    DiagnosticCode::Unknown,
                    format!("祖先 PID {pid} 在进程清单中缺失（可能已退出）"),
                )
            })
    };
    let ancestry_inspection = resolve_ancestry(target.pid(), now, &reader)?;
    let mut ancestry = ancestry_inspection.data().cloned().unwrap_or_default();
    let mut issues: Vec<DiagnosticIssue> = ancestry_inspection.issues;
    issues.extend(snapshot.issues.iter().cloned());

    // 2. 来源识别：证据采集（经端口）→ 纯判定链。
    let evidence: SourceEvidence = ports.evidence.evidence(&ancestry);
    let source = detect_source(&ancestry, &evidence);

    rewrite_lxc_runtime(&source, &mut ancestry);

    // 3. 目标进程选取：链尾即目标；ResolvedTarget 取 Command（空回退 unknown）。
    let Some(target_summary) = ancestry.last().cloned() else {
        return Err(InspectError::NotFound {
            subject: format!("PID {}", target.pid()),
        });
    };
    let resolved_target = if target_summary.command.is_empty() {
        String::from("unknown")
    } else {
        target_summary.command.clone()
    };

    // 4. 容器健康检查补全（parity：仅容器进程触发，探针由平台实现）。
    let mut target_summary = target_summary;
    complete_healthcheck(&mut target_summary, ports.healthcheck);

    // 5. 子进程快照：复用第 0 步快照（失败静默降级为空，parity：err 非空时
    //    childPIDs 保持空，不告警）。
    let children = children_of(&target_summary, &snapshot_entries);

    // 6. 进程详情：身份比对失败（PID 复用）或进程退出 → 整体失败；
    //    其他错误降级为诊断，资源详情缺失继续。
    let details = match ports.details.details(target) {
        Ok(details) => {
            issues.extend(details.issues);
            details.data
        }
        Err(err @ (InspectError::ProcessChanged { .. } | InspectError::NotFound { .. })) => {
            return Err(err);
        }
        Err(err) => {
            issues.push(issue_from_error(&err));
            None
        }
    };

    // 7. Socket 采集：失败保留诊断与空数据，不抹掉其余结果。
    let sockets = ports.network.sockets_of(target.pid());
    let socket_entries = sockets.data.clone().unwrap_or_default();
    issues.extend(sockets.issues.iter().cloned());

    // 8. 进程级文件锁（additive 端口；未提供时空列表）。
    let file_locks = collect_file_locks(ports.process_locks, target.pid(), &mut issues);

    // 9. RestartCount：仅 systemd 来源，从 Source.details 的 NRestarts 解析
    //    （parity：AnalyzePID 的 NRestarts 分支；解析失败计 0）。
    let restart_count = if source.source_type() == SourceType::Systemd {
        source
            .details()
            .iter()
            .find(|(key, _)| key == "NRestarts")
            .and_then(|(_, value)| value.parse::<u64>().ok())
            .unwrap_or(0)
    } else {
        0
    };

    // 10. 告警生成（服务名取目标 cgroup 的 .service 单元，parity
    // serviceFromCgroup；scope 归 None）。
    let target_cgroup = evidence
        .cgroup_by_pid
        .iter()
        .find(|(pid, _)| *pid == target.pid())
        .map(|(_, text)| text.as_str());
    let service = target_cgroup.and_then(crate::service_unit_from_cgroup);
    let warning_context = crate::warnings::WarningsContext {
        target: &target_summary,
        sockets: &socket_entries,
        details: details.as_ref(),
        service,
        restart_count,
        source_type: source.source_type(),
        is_windows,
        now,
    };
    let warnings = crate::warnings::warnings(&warning_context);

    Ok(Inspection::with_captured_at(
        Some(Analysis {
            target: target_summary,
            ancestry,
            resolved_target,
            source,
            restart_count,
            children,
            sockets: socket_entries,
            file_locks,
            details,
            warnings,
        }),
        issues,
        now,
    ))
}

/// 容器标签一致性改写（parity：AnalyzePID 对 lxc-based 标签的重写）——来源
/// 已按祖先命令精化运行时（incus/lxd/lxc），把链上泛化 "lxc" 标签统一改写。
fn rewrite_lxc_runtime(source: &crate::model::source::Source, ancestry: &mut [ProcessSummary]) {
    if source.source_type() == SourceType::Container
        && let Some(name) = source.name()
        && matches!(name, "incus" | "lxd" | "lxc")
    {
        for entry in ancestry {
            if let Some(container) = entry.container.as_mut()
                && container.runtime() == "lxc"
            {
                container.set_runtime(name);
            }
        }
    }
}

/// 容器健康检查补全：仅容器进程且 ID 非空时触发（parity：仅 docker/podman
/// 可判定，探针由平台实现；未提供探针时不改写）。
fn complete_healthcheck(
    target: &mut ProcessSummary,
    probe: Option<&dyn ContainerHealthcheckProbe>,
) {
    if let Some(probe) = probe
        && let Some(container) = target.container.as_mut()
        && !container.container_id().is_empty()
    {
        let status = probe.healthcheck_status(container.container_id(), container.runtime());
        container.set_healthcheck(status);
    }
}

/// 进程级文件锁采集（additive `ProcessFileLocks`；未注入端口时为空列表）。
fn collect_file_locks(
    locks: Option<&dyn ProcessFileLocks>,
    pid: Pid,
    issues: &mut Vec<DiagnosticIssue>,
) -> Vec<FileLockEntry> {
    locks.map_or_else(Vec::new, |locks| {
        let inspection = locks.locks_of(pid);
        issues.extend(inspection.issues.iter().cloned());
        inspection.data.unwrap_or_default()
    })
}

/// 子进程收集：从第 0 步快照筛 PPID == 目标，按 PID 升序去重（parity：快照
/// 失败静默降级为空，不影响其余分析）。
fn children_of(target: &ProcessSummary, snapshot: &[ProcessSummary]) -> Vec<Pid> {
    let mut children: Vec<Pid> = snapshot
        .iter()
        .filter(|p| p.parent_pid == Some(target.identity.pid()))
        .map(|p| p.identity.pid())
        .collect();
    children.sort_unstable();
    children.dedup();
    children
}

/// 采集器错误 → 结构化诊断（NotFound/ProcessChanged 已在上游整体失败，
/// 不进入此映射）。
fn issue_from_error(err: &InspectError) -> DiagnosticIssue {
    let code = match err {
        InspectError::PermissionDenied { .. } => DiagnosticCode::PermissionDenied,
        InspectError::ExternalTool { .. } => DiagnosticCode::ExternalToolFailed,
        InspectError::Unsupported { .. } => DiagnosticCode::Unsupported,
        _ => DiagnosticCode::Unknown,
    };
    DiagnosticIssue::new(code, err.to_string())
}
