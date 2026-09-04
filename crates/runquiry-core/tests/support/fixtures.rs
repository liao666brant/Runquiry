//! fixture 封套 DTO、装载器与 `SocketEntry` 输入边界校验。
//!
//! 校验发生在 fixture 装载边界（不改公共领域模型）：后续平台真实输入
//! （B2 各采集器把原始数据转成 [`SocketEntry`] 时）必须复用同一规则，
//! 见 `tests/fixtures/README.md` 的「输入边界校验」一节。

// 测试 crate 非 lib 目标，support 模块不对外导出：unreachable_pub 不适用。
#![allow(unreachable_pub)]
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::de::DeserializeOwned;

use runquiry_core::{
    CapabilityStatus, DiagnosticIssue, Inspection, SocketEntry, validate_socket_entry,
};

/// fixture 快照封套：平台、场景、确定性时刻、generation、能力状态与部分成功语义。
#[derive(Debug, serde::Deserialize)]
pub struct FixtureEnvelope<T> {
    /// 合成平台名（`linux` / `macos` / `windows`）。
    pub platform: String,
    /// 合成场景名（如 `normal`、`pid_reuse`）。
    pub scenario: String,
    /// 快照采集时刻（相对 `UNIX_EPOCH` 的毫秒数，合成值）。
    pub captured_at_epoch_ms: u64,
    /// 请求代数（合成值，与 [`crate::support::Generation::FIXTURE`] 一致）。
    pub generation: u64,
    /// 平台能力状态；场景不涉及能力表达时缺省。
    #[serde(default)]
    pub capability: Option<CapabilityStatus>,
    /// 快照数据；完全失败时为 `None`。
    pub data: Option<T>,
    /// 结构化诊断；单个 issue 不抹掉已取得数据。
    #[serde(default)]
    pub issues: Vec<DiagnosticIssue>,
}

/// 装载完成的 fixture：封套元数据 + 领域 [`Inspection`]。
#[derive(Debug)]
pub struct LoadedFixture<T> {
    /// 合成平台名。
    pub platform: String,
    /// 合成场景名。
    pub scenario: String,
    /// 确定性采集时刻。
    pub captured_at: SystemTime,
    /// 请求代数。
    pub generation: u64,
    /// 平台能力状态。
    pub capability: Option<CapabilityStatus>,
    /// 数据与诊断并存的部分成功容器。
    pub inspection: Inspection<T>,
}

/// fixture 根目录（workspace 根 `tests/fixtures/`）。
#[must_use]
pub fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

/// 从 workspace 根 `tests/fixtures/` 装载并反序列化一个快照封套。
///
/// # Errors
/// 文件缺失、JSON 损坏或字段不符合领域类型时返回带路径的错误说明。
pub fn load<T: DeserializeOwned>(relative: &str) -> Result<LoadedFixture<T>, String> {
    let path = fixtures_root().join(relative);
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("读取 {} 失败：{e}", path.display()))?;
    let envelope: FixtureEnvelope<T> =
        serde_json::from_str(&text).map_err(|e| format!("解析 {} 失败：{e}", path.display()))?;
    let captured_at = SystemTime::UNIX_EPOCH
        .checked_add(Duration::from_millis(envelope.captured_at_epoch_ms))
        .ok_or_else(|| format!("{} 的 captured_at_epoch_ms 越界", path.display()))?;
    Ok(LoadedFixture {
        inspection: Inspection::with_captured_at(envelope.data, envelope.issues, captured_at),
        platform: envelope.platform,
        scenario: envelope.scenario,
        captured_at,
        generation: envelope.generation,
        capability: envelope.capability,
    })
}

/// 装载 socket 快照并执行输入边界校验。
///
/// # Errors
/// 封套装载失败或条目违反端口一致性规则时返回错误。
pub fn load_sockets(relative: &str) -> Result<LoadedFixture<Vec<SocketEntry>>, String> {
    let loaded = load::<Vec<SocketEntry>>(relative)?;
    if let Some(entries) = loaded.inspection.data.as_ref() {
        validate_socket_entries(entries)?;
    }
    Ok(loaded)
}

/// `SocketEntry` 输入边界校验：TCP/TCP6/UDP/UDP6 必须携带合法端口
/// （`Some` 且 1..=65535）；Unix socket 必须没有端口（`None`）。
///
/// 规则已提升为 runquiry-core 公共 API [`runquiry_core::validate_socket_entry`]
/// （`tests/fixtures/README.md` 第 4 节要求），此处逐条委托，不再维护第二份
/// 实现；平台真实输入（B2 采集器）必须复用同一公共函数。
///
/// # Errors
/// 任一条目违反配对规则时返回说明哪个条目违规的错误。
pub fn validate_socket_entries(entries: &[SocketEntry]) -> Result<(), String> {
    for entry in entries {
        validate_socket_entry(entry)?;
    }
    Ok(())
}
