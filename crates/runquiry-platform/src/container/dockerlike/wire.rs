//! docker-like 家族的 wire 格式：`ps` JSON 条目、标签双形态与逐行/数组解析。
//!
//! 解析边界之后不再有 `serde_json::Value`：条目一律转为强类型 [`DockerLikeEntry`]。

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use runquiry_core::{ContainerKey, ContainerSummary, DiagnosticCode, DiagnosticIssue};

use super::super::parse::health_from_status;
use super::ListedContainer;
/// `ps` JSON 条目（docker 逐行 / podman、nerdctl 数组共用的字段集，对齐 witr
/// dockerLikeList 的 ID/Names/Image/Command/State/Status/CreatedAt/Labels）。
#[derive(Debug, Deserialize)]
pub(super) struct DockerLikeEntry {
    /// 容器 ID（`--no-trunc` 下为长 ID）。
    #[serde(rename = "ID")]
    id: String,
    /// 容器名称（docker/podman 为数组，nerdctl 可能为单字符串）。
    #[serde(rename = "Names", default, deserialize_with = "de_names")]
    names: Vec<String>,
    /// 镜像引用。
    #[serde(rename = "Image", default, deserialize_with = "de_string_or_array")]
    image: Option<String>,
    /// 入口命令（witr 字段集对齐，但快照无对应字段，不进入摘要）。
    #[serde(rename = "Command", default, deserialize_with = "de_string_or_array")]
    _command: Option<String>,
    /// 运行状态（如 `running`）。
    #[serde(rename = "State", default)]
    state: Option<String>,
    /// 人类可读状态（如 `Up 4 minutes (healthy)`）。
    #[serde(rename = "Status", default)]
    status: Option<String>,
    /// 标签集（对象或 `k=v,k2=v2` 文本，witr parseLabelString 兼容）。
    #[serde(rename = "Labels", default, deserialize_with = "de_labels")]
    labels: Option<Labels>,
}

/// 标签的两种机器形态。
#[derive(Debug)]
enum Labels {
    /// 对象形态（docker `{{json .}}` / podman）。
    Map(BTreeMap<String, String>),
    /// `k=v,k2=v2` 文本形态。
    Text(String),
}

impl Labels {
    /// 读取一个标签值。
    fn get(&self, key: &str) -> Option<String> {
        match self {
            Self::Map(map) => map.get(key).cloned(),
            Self::Text(text) => parse_label_text(text).remove(key),
        }
    }
}

/// 解析 `k=v,k2=v2` 文本（对齐 witr parseLabelString：按逗号切分、首个 `=` 分键值）。
fn parse_label_text(text: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for pair in text.split(',') {
        let pair = pair.trim();
        if let Some(idx) = pair.find('=')
            && idx > 0
        {
            map.insert(pair[..idx].to_string(), pair[idx + 1..].to_string());
        }
    }
    map
}

pub(super) fn parse_line_delimited(
    runtime: &str,
    stdout: &[u8],
) -> Result<Vec<DockerLikeEntry>, DiagnosticIssue> {
    let mut entries = Vec::new();
    for line in stdout.split(|byte: &u8| *byte == b'\n') {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match serde_json::from_slice(line) {
            Ok(entry) => entries.push(entry),
            Err(err) => return Err(parse_failed(runtime, &err)),
        }
    }
    Ok(entries)
}

/// JSON 数组（podman / nerdctl）；全空白视为空列表。
pub(super) fn parse_array(
    runtime: &str,
    stdout: &[u8],
) -> Result<Vec<DockerLikeEntry>, DiagnosticIssue> {
    if stdout.iter().all(u8::is_ascii_whitespace) {
        return Ok(Vec::new());
    }
    serde_json::from_slice(stdout).map_err(|err| parse_failed(runtime, &err))
}

fn parse_failed(runtime: &str, err: &serde_json::Error) -> DiagnosticIssue {
    DiagnosticIssue::new(
        DiagnosticCode::ParseFailed,
        format!("{runtime} 列表输出解析失败：{err}"),
    )
}

/// 列表条目 → 去重键 + 快照 + Compose 临时匹配键（不持久化、不进 UI）。
pub(super) fn to_listed(runtime: &str, entry: DockerLikeEntry) -> ListedContainer {
    let status = entry.status.clone().or_else(|| entry.state.clone());
    let health = entry.status.as_deref().and_then(health_from_status);
    let summary = ContainerSummary {
        key: ContainerKey {
            runtime: runtime.to_string(),
            id: entry.id.trim().to_string(),
        },
        name: entry.names.first().map(|name| name.trim().to_string()),
        image: entry.image.map(|image| image.trim().to_string()),
        status: status.map(|status| status.trim().to_string()),
        health,
        host_pid: None,
        started_at: None,
    };
    ListedContainer {
        summary,
        compose_project: entry
            .labels
            .as_ref()
            .and_then(|labels| labels.get("com.docker.compose.project")),
        compose_service: entry
            .labels
            .as_ref()
            .and_then(|labels| labels.get("com.docker.compose.service")),
    }
}

/// `Names`：数组、单字符串或 null。
fn de_names<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Value::deserialize(deserializer)? {
        Value::Null => Ok(Vec::new()),
        Value::String(text) => Ok(vec![text]),
        Value::Array(items) => items
            .into_iter()
            .map(|item| match item {
                Value::String(text) => Ok(text),
                _ => Err(serde::de::Error::custom("Names 数组元素必须是字符串")),
            })
            .collect(),
        _ => Err(serde::de::Error::custom("Names 必须是字符串或字符串数组")),
    }
}

/// 字符串或字符串数组（数组元素以空格连接，podman `Command` 形态）。
fn de_string_or_array<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Value::deserialize(deserializer)? {
        Value::Null => Ok(None),
        Value::String(text) => Ok(Some(text)),
        Value::Array(items) => {
            let mut parts = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Value::String(text) => parts.push(text),
                    _ => return Err(serde::de::Error::custom("数组元素必须是字符串")),
                }
            }
            Ok(Some(parts.join(" ")))
        }
        _ => Err(serde::de::Error::custom("必须是字符串或字符串数组")),
    }
}

/// 标签对象或 `k=v,k2=v2` 文本。
fn de_labels<'de, D>(deserializer: D) -> Result<Option<Labels>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Value::deserialize(deserializer)? {
        Value::Null => Ok(None),
        Value::Object(map) => {
            let mut labels = BTreeMap::new();
            for (key, value) in map {
                let Value::String(text) = value else {
                    return Err(serde::de::Error::custom("标签值必须是字符串"));
                };
                labels.insert(key, text);
            }
            Ok(Some(Labels::Map(labels)))
        }
        Value::String(text) => Ok(Some(Labels::Text(text))),
        _ => Err(serde::de::Error::custom("Labels 必须是对象或 k=v 文本")),
    }
}
