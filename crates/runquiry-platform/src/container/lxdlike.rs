//! Incus / LXD 共享的列表、主机 PID 实现（lxd-like 家族，parity §7）。
//!
//! 两者共用同一 REST JSON 结构（Incus 是 LXD 的分支）；主机 PID 经
//! `list <name> --format json` 的 `state.pid` 取得。Incus/LXD 的网络与挂载
//! 富化不在本轮范围（parity：容器富化 Compose 信息之外 out of scope），
//! 因此 enrich 为 no-op。

// 容器子模块为私有模块，pub(crate) 是父模块可见的最小可见性（仓库约定豁免）。
#![allow(clippy::redundant_pub_crate)]

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::command::{CommandFailure, StdCommandRunner};
use runquiry_core::{
    CommandOutput, CommandSpec, ContainerKey, ContainerSummary, DiagnosticCode, DiagnosticIssue,
    InspectError, Pid,
    port::command::{DETAIL_TIMEOUT, LIST_TIMEOUT},
};

use super::{ContainerEnrichment, ListedContainer};

/// `list --format json` 的实例条目（witr lxdLikeInstance 的 `ContainerSummary` 子集）。
#[derive(Debug, Deserialize)]
struct LxdInstance {
    /// 实例名（同时充当容器 ID）。
    name: String,
    /// 人类可读状态（如 `Running`）。
    #[serde(default)]
    status: Option<String>,
    /// 镜像来源（config 的 image.* 键）。
    #[serde(default)]
    config: BTreeMap<String, String>,
    /// 运行时状态（含主机 PID）。
    #[serde(default)]
    state: LxdState,
}

#[derive(Debug, Default, Deserialize)]
struct LxdState {
    /// 实例主进程在主机上的 PID；未运行时为 0。
    #[serde(default)]
    pid: Option<i64>,
}

/// 列出容器；成功返回条目与附加诊断（如 stderr 截断），失败返回单条诊断。
pub(crate) fn list(
    program: &str,
    runner: StdCommandRunner,
    runtime: &'static str,
) -> Result<(Vec<ListedContainer>, Vec<DiagnosticIssue>), DiagnosticIssue> {
    let spec = CommandSpec::new(program, ["list", "--format", "json"]);
    let output = run_for_list(runtime, runner, &spec)?;
    // 截断判定优先于退出码：超限 kill 后子进程退出码必然不可得（None）。
    if output.stdout_truncated {
        return Err(DiagnosticIssue::new(
            DiagnosticCode::OutputLimitExceeded,
            format!("{runtime} 列表输出超过上限被截断，结果不可信"),
        ));
    }
    require_exit_zero(runtime, &output)?;
    let mut issues = Vec::new();
    if output.stderr_truncated {
        issues.push(DiagnosticIssue::new(
            DiagnosticCode::OutputLimitExceeded,
            format!("{runtime} 列表命令的 stderr 超过上限被截断"),
        ));
    }
    let instances: Vec<LxdInstance> = if output.stdout.iter().all(u8::is_ascii_whitespace) {
        Vec::new()
    } else {
        serde_json::from_slice(&output.stdout).map_err(|err| parse_failed(runtime, &err))?
    };
    let items = instances
        .into_iter()
        .map(|instance| to_listed(runtime, &instance))
        .collect();
    Ok((items, issues))
}

/// 解析实例主进程在主机上的 PID（`list <name> --format json` 的 state.pid；
/// `DETAIL_TIMEOUT；parity：名称精确命中优先，否则取首条`）。
pub(crate) fn host_pid(
    program: &str,
    runner: StdCommandRunner,
    runtime: &str,
    id: &str,
) -> Result<Option<Pid>, InspectError> {
    let spec = CommandSpec::new(program, ["list", id, "--format", "json"]);
    let output = runner
        .run_classified(&spec, DETAIL_TIMEOUT)
        .map_err(CommandFailure::into_inspect_error)?;
    if output.exit_code != Some(0) {
        return Err(InspectError::ExternalTool {
            program: program.to_string(),
            detail: format!("{} list 退出码 {:?}", runtime, output.exit_code),
        });
    }
    let instances: Vec<LxdInstance> =
        serde_json::from_slice(&output.stdout).map_err(|err| InspectError::ExternalTool {
            program: program.to_string(),
            detail: format!("解析 list 输出失败：{err}"),
        })?;
    let pid = instances
        .iter()
        .find(|instance| instance.name == id)
        .or_else(|| instances.first())
        .and_then(|instance| instance.state.pid);
    Ok(pid
        .filter(|pid| *pid > 0)
        .and_then(|pid| u32::try_from(pid).ok())
        .and_then(|value| Pid::new(value).ok()))
}

/// 富集：Incus/LXD 可富集的网络与挂载字段不在 [`ContainerSummary`] 中，no-op。
/// 富集：Incus/LXD 可富集的网络与挂载字段不在 [`ContainerSummary`] 中，返回空富集。
pub(crate) fn enrich() -> ContainerEnrichment {
    ContainerEnrichment::default()
}

fn run_for_list(
    runtime: &str,
    runner: StdCommandRunner,
    spec: &CommandSpec,
) -> Result<CommandOutput, DiagnosticIssue> {
    match runner.run_classified(spec, LIST_TIMEOUT) {
        Ok(output) => Ok(output),
        Err(CommandFailure::Spawn { detail, .. }) => Err(DiagnosticIssue::new(
            DiagnosticCode::ExternalToolFailed,
            format!("{runtime} 列表命令无法启动：{detail}"),
        )),
        Err(CommandFailure::Timeout { timeout, .. }) => Err(DiagnosticIssue::new(
            DiagnosticCode::Timeout,
            format!("{} 列表命令超时（{}ms）", runtime, timeout.as_millis()),
        )),
    }
}

fn require_exit_zero(runtime: &str, output: &CommandOutput) -> Result<(), DiagnosticIssue> {
    if output.exit_code == Some(0) {
        return Ok(());
    }
    Err(DiagnosticIssue::new(
        DiagnosticCode::ExternalToolFailed,
        format!("{} 列表命令退出码 {:?}", runtime, output.exit_code),
    ))
}

fn parse_failed(runtime: &str, err: &serde_json::Error) -> DiagnosticIssue {
    DiagnosticIssue::new(
        DiagnosticCode::ParseFailed,
        format!("{runtime} 列表输出解析失败：{err}"),
    )
}

/// 实例条目 → 去重键 + 快照（lxd-like 无 Compose 临时键；列表已含 state.pid）。
fn to_listed(runtime: &str, instance: &LxdInstance) -> ListedContainer {
    let summary = ContainerSummary {
        key: ContainerKey {
            runtime: runtime.to_string(),
            id: instance.name.trim().to_string(),
        },
        name: Some(instance.name.trim().to_string()),
        image: image_from_config(&instance.config),
        status: instance
            .status
            .as_deref()
            .map(str::trim)
            .map(str::to_string),
        health: None,
        host_pid: instance
            .state
            .pid
            .filter(|pid| *pid > 0)
            .and_then(|pid| u32::try_from(pid).ok())
            .and_then(|value| Pid::new(value).ok()),
        started_at: None,
    };
    ListedContainer {
        summary,
        compose_project: None,
        compose_service: None,
    }
}

/// 镜像描述：`image.description` 优先，否则 `image.os + " " + image.release`。
fn image_from_config(config: &BTreeMap<String, String>) -> Option<String> {
    if let Some(description) = config.get("image.description")
        && !description.trim().is_empty()
    {
        return Some(description.trim().to_string());
    }
    let os = config.get("image.os").map_or("", String::as_str);
    if os.is_empty() {
        return None;
    }
    let release = config.get("image.release").map_or("", String::as_str);
    Some(
        format!("{} {}", os.trim(), release.trim())
            .trim()
            .to_string(),
    )
}
