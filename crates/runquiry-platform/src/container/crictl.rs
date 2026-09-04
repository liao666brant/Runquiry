//! crictl 运行时（runtime 名 `k8s`，显示名 k8s；parity §7）。
//!
//! 列表：`crictl ps -o json`；主机 PID 与富集：`crictl inspect <id>`。
//! inspect 兼容老版本把 `info` 字段包装成 JSON 字符串的形态
//! （witr crictlInspect 的双解析语义）。

// 容器子模块为私有模块，pub(crate) 是父模块可见的最小可见性（仓库约定豁免）。
#![allow(clippy::redundant_pub_crate)]

use serde::Deserialize;

use crate::command::{CommandFailure, StdCommandRunner};
use runquiry_core::{
    CommandOutput, CommandSpec, ContainerKey, ContainerSummary, DiagnosticCode, DiagnosticIssue,
    InspectError, Pid,
    port::command::{DETAIL_TIMEOUT, LIST_TIMEOUT},
};

use super::parse::parse_machine_time;
use super::{ContainerEnrichment, ListedContainer};

/// 诊断前缀使用的运行时显示名（对齐 witr：crictl 显示为 k8s）。
const DISPLAY: &str = "k8s";

/// `crictl ps -o json` 的解析结构（仅取 `ContainerSummary` 所需字段）。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CrictlList {
    /// 容器条目。
    #[serde(default)]
    containers: Vec<CrictlContainer>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CrictlContainer {
    /// 容器 ID。
    id: String,
    /// 名称元数据。
    #[serde(default)]
    metadata: Option<CrictlMetadata>,
    /// 镜像引用。
    #[serde(default)]
    image: Option<CrictlImage>,
    /// 状态（如 `CONTAINER_RUNNING`）。
    #[serde(default)]
    state: Option<String>,
    /// 创建时间（witr 语义：作为 crictl 容器的 `StartedAt` 使用）。
    #[serde(default)]
    created_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CrictlMetadata {
    /// 容器名。
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CrictlImage {
    /// 镜像引用。
    #[serde(default)]
    image: Option<String>,
}

/// `crictl inspect` 的直接形态（info 为对象）。
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CrictlInspect {
    /// 容器状态（含 startedAt）。
    #[serde(default)]
    status: CrictlStatus,
    /// 运行时信息（含主机 PID）。
    #[serde(default)]
    info: CrictlInfo,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CrictlStatus {
    /// 容器启动时间。
    #[serde(default)]
    started_at: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct CrictlInfo {
    /// 容器主进程在主机上的 PID。
    #[serde(default, rename = "pid")]
    pid: Option<i64>,
}

/// 老版本 crictl 的包装形态：`info` 是 JSON 编码字符串。
#[derive(Debug, Deserialize)]
struct CrictlInspectWrapper {
    /// 状态对象（原样保留后二次解析）。
    #[serde(default)]
    status: serde_json::Value,
    /// JSON 字符串包装的 info。
    #[serde(default)]
    info: Option<String>,
}

/// 列出容器；成功返回条目与附加诊断（如 stderr 截断），失败返回单条诊断。
pub(crate) fn list(
    program: &str,
    runner: StdCommandRunner,
) -> Result<(Vec<ListedContainer>, Vec<DiagnosticIssue>), DiagnosticIssue> {
    let spec = CommandSpec::new(program, ["ps", "-o", "json"]);
    let output = run_for_list(runner, &spec)?;
    if output.stdout_truncated {
        return Err(DiagnosticIssue::new(
            DiagnosticCode::OutputLimitExceeded,
            format!("{DISPLAY} 列表输出超过上限被截断，结果不可信"),
        ));
    }
    let mut issues = Vec::new();
    if output.stderr_truncated {
        issues.push(DiagnosticIssue::new(
            DiagnosticCode::OutputLimitExceeded,
            format!("{DISPLAY} 列表命令的 stderr 超过上限被截断"),
        ));
    }
    if output.exit_code != Some(0) {
        return Err(DiagnosticIssue::new(
            DiagnosticCode::ExternalToolFailed,
            format!("{DISPLAY} 列表命令退出码 {:?}", output.exit_code),
        ));
    }
    let parsed: CrictlList = if output.stdout.iter().all(u8::is_ascii_whitespace) {
        CrictlList {
            containers: Vec::new(),
        }
    } else {
        serde_json::from_slice(&output.stdout).map_err(|err| parse_failed(&err))?
    };
    let items = parsed.containers.into_iter().map(to_listed).collect();
    Ok((items, issues))
}

/// 解析容器在主机上的 PID（`crictl inspect` 的 `info.pid；DETAIL_TIMEOUT`）。
pub(crate) fn host_pid(
    program: &str,
    runner: StdCommandRunner,
    id: &str,
) -> Result<Option<Pid>, InspectError> {
    let inspect = inspect(program, runner, id)?;
    Ok(inspect
        .info
        .pid
        .filter(|pid| *pid > 0)
        .and_then(|pid| u32::try_from(pid).ok())
        .and_then(|value| Pid::new(value).ok()))
}

/// 富集容器：取 `status.startedAt`（witr `crictlRuntime.Enrich` 的 `StartedAt` 部分）。
pub(crate) fn enrich(
    program: &str,
    runner: StdCommandRunner,
    id: &str,
) -> Result<ContainerEnrichment, InspectError> {
    let inspect = inspect(program, runner, id)?;
    let started_at = inspect
        .status
        .started_at
        .as_deref()
        .and_then(parse_machine_time);
    Ok(ContainerEnrichment { started_at })
}

/// 共享的 `crictl inspect <id>` `调用（DETAIL_TIMEOUT；双解析兼容老版本包装`）。
fn inspect(
    program: &str,
    runner: StdCommandRunner,
    id: &str,
) -> Result<CrictlInspect, InspectError> {
    let spec = CommandSpec::new(program, ["inspect", id]);
    let output = runner
        .run_classified(&spec, DETAIL_TIMEOUT)
        .map_err(CommandFailure::into_inspect_error)?;
    if output.exit_code != Some(0) {
        return Err(InspectError::ExternalTool {
            program: program.to_string(),
            detail: format!("inspect 退出码 {:?}", output.exit_code),
        });
    }
    parse_inspect(&output.stdout).map_err(|detail| InspectError::ExternalTool {
        program: program.to_string(),
        detail: format!("解析 inspect 输出失败：{detail}"),
    })
}

/// 双解析：先按直接形态（info 为对象）；失败再按老版本包装形态
/// （info 为 JSON 编码字符串，witr crictlInspect 语义）。
fn parse_inspect(bytes: &[u8]) -> Result<CrictlInspect, String> {
    if let Ok(direct) = serde_json::from_slice::<CrictlInspect>(bytes) {
        return Ok(direct);
    }
    let wrapper: CrictlInspectWrapper =
        serde_json::from_slice(bytes).map_err(|err| err.to_string())?;
    let status = match wrapper.status {
        serde_json::Value::Null => CrictlStatus::default(),
        value => serde_json::from_value(value).map_err(|err| format!("status 字段：{err}"))?,
    };
    let info = match wrapper.info.as_deref() {
        None | Some("") => CrictlInfo::default(),
        Some(text) => serde_json::from_str(text).map_err(|err| format!("info 字段：{err}"))?,
    };
    Ok(CrictlInspect { status, info })
}

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

fn parse_failed(err: &serde_json::Error) -> DiagnosticIssue {
    DiagnosticIssue::new(
        DiagnosticCode::ParseFailed,
        format!("{DISPLAY} 列表输出解析失败：{err}"),
    )
}

/// 列表条目 → 去重键 + 快照（crictl 无 Compose 临时键）。
fn to_listed(entry: CrictlContainer) -> ListedContainer {
    // parity：witr 对 crictl 状态做 CONTAINER_ 前缀剥离。
    let state = entry.state.as_deref().map(|state| {
        state
            .strip_prefix("CONTAINER_")
            .unwrap_or(state)
            .to_string()
    });
    let summary = ContainerSummary {
        key: ContainerKey {
            runtime: "k8s".to_string(),
            id: entry.id.trim().to_string(),
        },
        name: entry
            .metadata
            .and_then(|meta| meta.name)
            .map(|name| name.trim().to_string()),
        image: entry
            .image
            .and_then(|img| img.image)
            .map(|image| image.trim().to_string()),
        status: state,
        health: None,
        host_pid: None,
        // witr 语义：crictl 的 `StartedAt` 取列表的 `createdAt`。
        started_at: entry.created_at.as_deref().and_then(parse_machine_time),
    };
    ListedContainer {
        summary,
        command: None,
        compose_project: None,
        compose_service: None,
    }
}
