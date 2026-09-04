//! Docker / Podman / nerdctl 共享的列表、主机 PID 与富集实现（docker-like 家族）。
//!
//! 机器格式（JSON 优先，parity §7）：
//! * 列表：`ps --no-trunc --format`；docker 为 `{{json .}}` 逐行 JSON，
//!   podman / nerdctl 为 `json` 数组；
//! * 主机 PID 与富集：`inspect --format {{json .State}}` 的
//!   `{"Pid":..,"StartedAt":".."}`（列表只有创建时间，StartedAt 由 enrich 补充，
//!   对齐 witr `dockerLikeEnrich` 语义）。
//!
//! Compose 标签（com.docker.compose.project / .service）只作为解析阶段临时
//! 匹配键随 [`ListedContainer`] 返回，不进入 [`ContainerSummary`]。

// 容器子模块为私有模块，pub(crate) 是父模块可见的最小可见性（仓库约定豁免）。
#![allow(clippy::redundant_pub_crate)]

use crate::command::{CommandFailure, StdCommandRunner};
use runquiry_core::{
    CommandOutput, CommandSpec, DiagnosticCode, DiagnosticIssue, HealthcheckStatus, InspectError,
    Pid,
    port::command::{DETAIL_TIMEOUT, LIST_TIMEOUT},
};

use serde::Deserialize;

use super::parse::parse_machine_time;
use super::{ContainerEnrichment, ListedContainer};
use wire::{DockerLikeEntry, parse_array, parse_line_delimited, to_listed};

mod wire;

/// docker-like 家族某二进制的调用形态。
pub(crate) struct DockerLikeBin {
    /// 程序名或绝对路径。
    pub program: String,
    /// 运行时名（[`ContainerKey::runtime`] 与诊断前缀）。
    pub runtime: &'static str,
    /// `ps --format` 模板：docker 为 `{{json .}}`，podman / nerdctl 为 `json`。
    pub list_format: &'static str,
    /// 列表输出形态：docker 逐行 JSON，podman / nerdctl 为 JSON 数组。
    pub line_delimited: bool,
}

/// `inspect --format {{json .State}}` 的解析结构。
#[derive(Debug, Deserialize)]
struct DockerLikeState {
    /// 容器主进程在主机上的 PID；0 / 缺失表示运行时无法给出。
    #[serde(rename = "Pid")]
    pid: Option<i64>,
    /// 容器主进程启动时间。
    #[serde(rename = "StartedAt")]
    started_at: Option<String>,
}

/// 逐行 JSON（docker `{{json .}}`）；空行跳过，任一行损坏即整体 `ParseFailed`。
/// 列出容器；成功返回条目与附加诊断（如 stderr 截断），失败返回单条诊断。
pub(crate) fn list(
    bin: &DockerLikeBin,
    runner: StdCommandRunner,
) -> Result<(Vec<ListedContainer>, Vec<DiagnosticIssue>), DiagnosticIssue> {
    let spec = CommandSpec::new(
        &bin.program,
        ["ps", "--no-trunc", "--format", bin.list_format],
    );
    let output = run_for_list(bin.runtime, runner, &spec)?;
    // 截断判定优先于退出码：超限 kill 后子进程退出码必然不可得（None）。
    if output.stdout_truncated {
        return Err(DiagnosticIssue::new(
            DiagnosticCode::OutputLimitExceeded,
            format!("{} 列表输出超过上限被截断，结果不可信", bin.runtime),
        ));
    }
    require_exit_zero(bin.runtime, &output)?;
    let mut issues = Vec::new();
    if output.stderr_truncated {
        issues.push(DiagnosticIssue::new(
            DiagnosticCode::OutputLimitExceeded,
            format!("{} 列表命令的 stderr 超过上限被截断", bin.runtime),
        ));
    }
    let entries: Vec<DockerLikeEntry> = if bin.line_delimited {
        parse_line_delimited(bin.runtime, &output.stdout)?
    } else {
        parse_array(bin.runtime, &output.stdout)?
    };
    let items = entries
        .into_iter()
        .map(|entry| to_listed(bin.runtime, entry))
        .collect();
    Ok((items, issues))
}

/// 解析容器在主机上的 PID（`inspect` 的 `State` `JSON；DETAIL_TIMEOUT`）。
pub(crate) fn host_pid(
    bin: &DockerLikeBin,
    runner: StdCommandRunner,
    id: &str,
) -> Result<Option<Pid>, InspectError> {
    let state = inspect_state(bin, runner, id)?;
    Ok(state_pid(state.pid))
}

/// 富集容器：取 `State.StartedAt`（列表只有创建时间；witr dockerLikeEnrich 语义）。
pub(crate) fn enrich(
    bin: &DockerLikeBin,
    runner: StdCommandRunner,
    id: &str,
) -> Result<ContainerEnrichment, InspectError> {
    let state = inspect_state(bin, runner, id)?;
    let started_at = state.started_at.as_deref().and_then(parse_machine_time);
    Ok(ContainerEnrichment { started_at })
}

/// 共享的 `inspect --format {{json .State}}` `调用（DETAIL_TIMEOUT`）。
fn inspect_state(
    bin: &DockerLikeBin,
    runner: StdCommandRunner,
    id: &str,
) -> Result<DockerLikeState, InspectError> {
    let spec = CommandSpec::new(
        &bin.program,
        ["inspect", "--format", "{{json .State}}", "--", id],
    );
    let output = runner
        .run_classified(&spec, DETAIL_TIMEOUT)
        .map_err(CommandFailure::into_inspect_error)?;
    if output.exit_code != Some(0) {
        return Err(InspectError::ExternalTool {
            program: bin.program.clone(),
            detail: format!("inspect 退出码 {:?}", output.exit_code),
        });
    }
    serde_json::from_slice(&output.stdout).map_err(|err| InspectError::ExternalTool {
        program: bin.program.clone(),
        detail: format!("解析 inspect 输出失败：{err}"),
    })
}

/// 主机 PID 归一：缺失 / 零 / 负值一律 `None`（不伪造）。
fn state_pid(pid: Option<i64>) -> Option<Pid> {
    let pid = pid?;
    if pid <= 0 {
        return None;
    }
    u32::try_from(pid)
        .ok()
        .and_then(|value| Pid::new(value).ok())
}

/// `inspect --format {{json .Config.Healthcheck}}` 的解析结构。
#[derive(Debug, Deserialize)]
struct DockerLikeHealthcheck {
    /// HEALTHCHECK 探针定义；未配置时为 `null` 或缺失。
    #[serde(rename = "Test", default)]
    test: Option<Vec<String>>,
}

/// 探测容器是否定义了 HEALTHCHECK（parity `ContainerHealthcheckStatus`：
/// 仅 docker/podman 可判定，由调用方先行限制 runtime；inspect 失败归
/// `None`——与 witr 空串语义一致，此时「无健康检查」告警不触发）。
pub(crate) fn healthcheck_config(
    bin: &DockerLikeBin,
    runner: StdCommandRunner,
    id: &str,
) -> Result<Option<HealthcheckStatus>, InspectError> {
    let spec = CommandSpec::new(
        &bin.program,
        [
            "inspect",
            "--format",
            "{{json .Config.Healthcheck}}",
            "--",
            id,
        ],
    );
    let output = runner
        .run_classified(&spec, DETAIL_TIMEOUT)
        .map_err(CommandFailure::into_inspect_error)?;
    if output.exit_code != Some(0) {
        return Err(InspectError::ExternalTool {
            program: bin.program.clone(),
            detail: format!("inspect 退出码 {:?}", output.exit_code),
        });
    }
    let config: Option<DockerLikeHealthcheck> =
        serde_json::from_slice(&output.stdout).map_err(|err| InspectError::ExternalTool {
            program: bin.program.clone(),
            detail: format!("解析 inspect 输出失败：{err}"),
        })?;
    // inspect 成功即有判定：Healthcheck 为 null / Test 缺失或空 → Absent
    // （parity：模板 else 分支的 "absent"）。
    let configured = config.is_some_and(|config| config.test.is_some_and(|test| !test.is_empty()));
    Ok(Some(if configured {
        HealthcheckStatus::Present
    } else {
        HealthcheckStatus::Absent
    }))
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

/// 非零退出码在调用方判定：列表命令非零退出视为该运行时失败（parity：witr
/// 出错即返回 nil），不使用部分输出。
fn require_exit_zero(runtime: &str, output: &CommandOutput) -> Result<(), DiagnosticIssue> {
    if output.exit_code == Some(0) {
        return Ok(());
    }
    Err(DiagnosticIssue::new(
        DiagnosticCode::ExternalToolFailed,
        format!("{} 列表命令退出码 {:?}", runtime, output.exit_code),
    ))
}
