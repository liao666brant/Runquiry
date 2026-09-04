//! 经典 LXC 运行时（runtime 名 `lxc`；parity §7）。
//!
//! 列表：`lxc-ls --fancy --format json`（无镜像与 PID 元数据，镜像为空、
//! PID 由 [`host_pid`] 惰性补齐）；主机 PID：`lxc-info -n <name> -p -H`。
//! 经典 LXC 不跟踪创建后镜像元数据，enrich 为 no-op。

// 容器子模块为私有模块，pub(crate) 是父模块可见的最小可见性（仓库约定豁免）。
#![allow(clippy::redundant_pub_crate)]

use serde::Deserialize;

use crate::command::{CommandFailure, StdCommandRunner};
use runquiry_core::{
    CommandOutput, CommandSpec, ContainerKey, ContainerSummary, DiagnosticCode, DiagnosticIssue,
    InspectError, Pid,
    port::command::{DETAIL_TIMEOUT, LIST_TIMEOUT},
};

use super::{ContainerEnrichment, ListedContainer};

/// `lxc-ls --fancy --format json` 的条目（ContainerSummary 所需子集）。
#[derive(Debug, Deserialize)]
struct LxcLsEntry {
    /// 容器名（同时充当容器 ID）。
    name: String,
    /// 状态（如 `RUNNING`）。
    #[serde(default)]
    state: Option<String>,
}

/// 列出容器；成功返回条目与附加诊断，失败返回单条诊断。
pub(crate) fn list(
    program: &str,
    runner: StdCommandRunner,
) -> Result<(Vec<ListedContainer>, Vec<DiagnosticIssue>), DiagnosticIssue> {
    let spec = CommandSpec::new(program, ["--fancy", "--format", "json"]);
    let output = run_for_list(runner, &spec)?;
    // 截断判定优先于退出码：超限 kill 后子进程退出码必然不可得（None）。
    if output.stdout_truncated {
        return Err(DiagnosticIssue::new(
            DiagnosticCode::OutputLimitExceeded,
            format!("{DISPLAY} 列表输出超过上限被截断，结果不可信"),
        ));
    }
    require_exit_zero(&output)?;
    let mut issues = Vec::new();
    if output.stderr_truncated {
        issues.push(DiagnosticIssue::new(
            DiagnosticCode::OutputLimitExceeded,
            format!("{DISPLAY} 列表命令的 stderr 超过上限被截断"),
        ));
    }
    let entries: Vec<LxcLsEntry> = if output.stdout.iter().all(u8::is_ascii_whitespace) {
        Vec::new()
    } else {
        serde_json::from_slice(&output.stdout).map_err(|err| parse_failed(&err))?
    };
    let items = entries.iter().map(to_listed).collect();
    Ok((items, issues))
}

/// 解析容器主进程在主机上的 PID（`lxc-info -n <name> -p -H`；`DETAIL_TIMEOUT`）。
pub(crate) fn host_pid(
    program: &str,
    runner: StdCommandRunner,
    id: &str,
) -> Result<Option<Pid>, InspectError> {
    let spec = CommandSpec::new(program, ["-n", id, "-p", "-H"]);
    let output = runner
        .run_classified(&spec, DETAIL_TIMEOUT)
        .map_err(CommandFailure::into_inspect_error)?;
    if output.exit_code != Some(0) {
        return Err(InspectError::ExternalTool {
            program: program.to_string(),
            detail: format!("lxc-info 退出码 {:?}", output.exit_code),
        });
    }
    // `-p -H` 输出单个 PID 文本；解析失败或非正数一律视为不可得，不伪造。
    let text = String::from_utf8_lossy(&output.stdout);
    let pid = text.trim().parse::<i64>().ok();
    Ok(pid
        .filter(|pid| *pid > 0)
        .and_then(|pid| u32::try_from(pid).ok())
        .and_then(|value| Pid::new(value).ok()))
}

/// 富集：经典 LXC 无额外可富集字段，no-op。
/// 富集：经典 LXC 无额外可富集字段，返回空富集。
pub(crate) fn enrich() -> ContainerEnrichment {
    ContainerEnrichment::default()
}

/// 诊断前缀（对齐 witr：经典 LXC 显示为 lxc）。
const DISPLAY: &str = "lxc";

fn run_for_list(
    runner: StdCommandRunner,
    spec: &CommandSpec,
) -> Result<CommandOutput, DiagnosticIssue> {
    match runner.run_classified(spec, LIST_TIMEOUT) {
        Ok(output) => Ok(output),
        Err(CommandFailure::Spawn { detail, .. }) => Err(DiagnosticIssue::new(
            DiagnosticCode::ExternalToolFailed,
            format!("{DISPLAY} 列表命令无法启动：{detail}"),
        )),
        Err(CommandFailure::Timeout { timeout, .. }) => Err(DiagnosticIssue::new(
            DiagnosticCode::Timeout,
            format!("{} 列表命令超时（{}ms）", DISPLAY, timeout.as_millis()),
        )),
        Err(CommandFailure::OutputLimit { .. }) => Err(DiagnosticIssue::new(
            DiagnosticCode::OutputLimitExceeded,
            format!("{DISPLAY} 列表命令输出超过上限"),
        )),
        Err(CommandFailure::Cancelled { .. }) => Err(DiagnosticIssue::new(
            DiagnosticCode::ExternalToolFailed,
            format!("{DISPLAY} 列表命令已取消"),
        )),
    }
}

fn require_exit_zero(output: &CommandOutput) -> Result<(), DiagnosticIssue> {
    if output.exit_code == Some(0) {
        return Ok(());
    }
    Err(DiagnosticIssue::new(
        DiagnosticCode::ExternalToolFailed,
        format!("{} 列表命令退出码 {:?}", DISPLAY, output.exit_code),
    ))
}

fn parse_failed(err: &serde_json::Error) -> DiagnosticIssue {
    DiagnosticIssue::new(
        DiagnosticCode::ParseFailed,
        format!("{DISPLAY} 列表输出解析失败：{err}"),
    )
}

/// 条目 → 去重键 + 快照（经典 LXC 无镜像/PID 元数据；无 Compose 临时键）。
fn to_listed(entry: &LxcLsEntry) -> ListedContainer {
    let summary = ContainerSummary {
        key: ContainerKey {
            runtime: "lxc".to_string(),
            id: entry.name.trim().to_string(),
        },
        name: Some(entry.name.trim().to_string()),
        image: None,
        status: entry.state.as_deref().map(str::trim).map(str::to_string),
        health: None,
        host_pid: None,
        started_at: None,
    };
    ListedContainer {
        summary,
        command: None,
        compose_project: None,
        compose_service: None,
    }
}
