//! 告警规则（parity §6：witr `internal/source/detect.go` 的 `Warnings`、
//! `network.go` 的 `IsPublicBind`、`envSuspiciousWarnings` 与相关名单）。
//!
//! 顺序契约：告警条目的 append 顺序与 witr 函数体逐条一致、确定性——
//! 服务重启 → 健康状态 → 公开监听 → root / 危险 capabilities（互斥）→
//! 未知来源 → 长运行 → 可疑工作目录 → 容器无健康检查 → 服务名不匹配 →
//! 已删除二进制 → 可疑环境变量（`LD_PRELOAD` 先于 `DYLD_*`，`DYLD` 键排序）。
//! 消息为 witr 英文原文（展示层对齐，不承担控制流；分支判定用
//! [`WarningKind`]）。

use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::model::container_context::HealthcheckStatus;
use crate::model::health::HealthStatus;
use crate::model::process::{ProcessDetails, ProcessSummary};
use crate::model::source::SourceType;
use crate::port::network::SocketEntry;

/// 告警类别（稳定判据，供测试与 UI 分支；message 仅展示）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WarningKind {
    /// 服务重启次数过多（systemd `NRestarts` > 5）。
    ServiceRestart,
    /// 僵尸进程。
    Zombie,
    /// 已停止（T 状态）。
    Stopped,
    /// 高 CPU（witr 阈值：累计 CPU > 2h）。
    HighCpu,
    /// 高内存（witr 阈值：RSS > 1GB）。
    HighMem,
    /// 公网接口监听。
    PublicListen,
    /// root 运行。
    RootUser,
    /// 危险 capabilities。
    DangerousCapabilities,
    /// 未知来源（Windows 豁免）。
    UnknownSource,
    /// 长运行（> 90 天）。
    LongRunning,
    /// 可疑工作目录。
    SuspiciousWorkingDir,
    /// 容器未配置健康检查。
    ContainerNoHealthcheck,
    /// 服务名与进程名不匹配。
    ServiceNameMismatch,
    /// 已删除二进制。
    DeletedBinary,
    /// 可疑环境变量（库注入面）。
    SuspiciousEnv,
}

/// 单条告警：稳定类别 + 面向展示的 message（witr 英文原文）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Warning {
    kind: WarningKind,
    message: String,
}

impl Warning {
    /// 构造一条告警。
    pub const fn new(kind: WarningKind, message: String) -> Self {
        Self { kind, message }
    }

    /// 读取告警类别。
    pub const fn kind(&self) -> WarningKind {
        self.kind
    }

    /// 读取展示文案。
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// 告警输入上下文：链尾进程 + 判定所需输入（缺省输入不触发对应告警）。
#[derive(Debug, Clone)]
pub struct WarningsContext<'a> {
    /// 链尾（目标）进程摘要。
    pub target: &'a ProcessSummary,
    /// 目标进程 Socket（公开监听告警输入）。
    pub sockets: &'a [SocketEntry],
    /// 进程详情（工作目录与环境变量来源）；采集失败为 `None`，对应告警
    /// 不触发（与 witr 详情字段缺失行为一致）。
    pub details: Option<&'a ProcessDetails>,
    /// 服务名（cgroup 解析出的 systemd .service 单元；非服务为 `None`）。
    pub service: Option<String>,
    /// 服务重启计数（仅 systemd 来源有效，其余传 0）。
    pub restart_count: u64,
    /// 已判定的来源类型（调用方传入，parity：srcType 优先于重新 Detect）。
    pub source_type: SourceType,
    /// 是否 Windows 平台（未知来源告警豁免；parity：runtime.GOOS 分支）。
    pub is_windows: bool,
    /// 注入时钟：长运行告警的"现在"，测试显式给定，不读墙钟。
    pub now: SystemTime,
}

/// 生成告警列表：顺序与 witr `Warnings` 函数体 append 顺序逐条一致、确定性。
///
/// 只基于链尾（目标）进程判定（parity：`last := p[len(p)-1]`）；来源类型
/// 由调用方传入（管线已判定，避免重复探测）。
#[must_use]
pub fn warnings(context: &WarningsContext<'_>) -> Vec<Warning> {
    let mut result: Vec<Warning> = Vec::new();
    push_restart_and_health_warnings(context, &mut result);
    push_exposure_warnings(context, &mut result);
    push_context_warnings(context, &mut result);
    result
}

/// 长运行阈值（parity：`> 90 * 24 小时`，严格大于）。
const LONG_RUNNING_THRESHOLD: Duration = Duration::from_hours(2_160);

/// 服务重启与健康状态告警（witr 顺序第 1-2 位）。
fn push_restart_and_health_warnings(context: &WarningsContext<'_>, result: &mut Vec<Warning>) {
    let last = context.target;
    // 服务重启告警（parity：restartCount > 5；计数来自服务管理器，未知为 0）。
    if context.restart_count > 5 {
        result.push(Warning::new(
            WarningKind::ServiceRestart,
            format!("Service has restarted {} times", context.restart_count),
        ));
    }
    // 健康状态告警（parity：Health 标签 switch 分支）。
    match last.health {
        HealthStatus::Zombie => result.push(Warning::new(
            WarningKind::Zombie,
            String::from("Process is a zombie (defunct)"),
        )),
        HealthStatus::Stopped => result.push(Warning::new(
            WarningKind::Stopped,
            String::from("Process is stopped (T state)"),
        )),
        HealthStatus::HighCpu => result.push(Warning::new(
            WarningKind::HighCpu,
            String::from("Process is using high CPU (>2h total)"),
        )),
        HealthStatus::HighMem => result.push(Warning::new(
            WarningKind::HighMem,
            String::from("Process is using high memory (>1GB RSS)"),
        )),
        HealthStatus::Healthy | HealthStatus::Unknown => {}
    }
}

/// 暴露面与身份类告警（witr 顺序第 3-6：公开监听、root / 危险 capabilities、
/// 未知来源、长运行）。
fn push_exposure_warnings(context: &WarningsContext<'_>, result: &mut Vec<Warning>) {
    let last = context.target;

    // 公开监听告警（parity：IsPublicBind——仅 LISTEN 且绑定 any 地址）。
    if is_public_bind(context.sockets) {
        result.push(Warning::new(
            WarningKind::PublicListen,
            String::from("Process is listening on a public interface"),
        ));
    }

    // root 与危险 capabilities 互斥（parity：else if 分支）。
    if last.user.as_deref() == Some("root") {
        result.push(Warning::new(
            WarningKind::RootUser,
            String::from("Process is running as root"),
        ));
    } else if !last.capabilities.is_empty() {
        let dangerous: Vec<&str> = last
            .capabilities
            .iter()
            .filter(|c| DANGEROUS_CAPABILITIES.contains(&c.as_str()))
            .map(String::as_str)
            .collect();
        if !dangerous.is_empty() {
            result.push(Warning::new(
                WarningKind::DangerousCapabilities,
                format!(
                    "Process has dangerous capabilities: {}",
                    dangerous.join(", ")
                ),
            ));
        }
    }

    // 未知来源告警（parity：Windows 豁免——孤儿进程链在 Windows 是常态，
    //    不是可靠的「无监管」信号）。
    if context.source_type == SourceType::Unknown && !context.is_windows {
        result.push(Warning::new(
            WarningKind::UnknownSource,
            String::from("No known supervisor or service manager detected"),
        ));
    }

    // 长运行告警（parity：>90*24h 严格大于；启动时刻不可得跳过，避免把
    // 「读不到启动时间」误报为「远古进程」）。
    if let Some(started) = last.identity.start_time()
        && let Ok(age) = context.now.duration_since(started)
        && age > LONG_RUNNING_THRESHOLD
    {
        result.push(Warning::new(
            WarningKind::LongRunning,
            String::from("Process has been running for over 90 days"),
        ));
    }
}

/// 上下文类告警（witr 顺序第 7-11：可疑目录、容器健康检查、服务名不匹配、
/// 已删除二进制、可疑环境变量）。
fn push_context_warnings(context: &WarningsContext<'_>, result: &mut Vec<Warning>) {
    let last = context.target;

    // 可疑工作目录（parity：suspiciousDirs 名单 /、/tmp、/var/tmp；工作
    // 目录不可得时按 witr 的 "unknown" 占位——不在名单内，不告警）。
    if let Some(dir) = context
        .details
        .and_then(|details| details.working_dir.as_ref())
    {
        let dir_text = dir.to_string_lossy();
        if SUSPICIOUS_DIRS.contains(&dir_text.as_ref()) {
            result.push(Warning::new(
                WarningKind::SuspiciousWorkingDir,
                format!("Process is running from a suspicious working directory: {dir_text}"),
            ));
        }
    }

    // 容器无健康检查告警：仅运行时确认未配置（absent）时触发；unknown /
    // 未探测 / 非 Linux 不告警（parity 注释）。
    if let Some(healthcheck) = last
        .container
        .as_ref()
        .and_then(crate::model::container_context::ContainerContext::healthcheck)
        && healthcheck == HealthcheckStatus::Absent
    {
        result.push(Warning::new(
            WarningKind::ContainerNoHealthcheck,
            String::from("Container has no healthcheck configured"),
        ));
    }

    // 服务名与进程名不匹配（parity：svcCore 处理段——剥单元后缀、模板
    // 取 @ 前基名、小写后双向包含任一成立即视为相关）。
    if let Some(service) = context.service.as_ref()
        && !service.is_empty()
        && !last.command.is_empty()
    {
        let mut svc_core = service.clone();
        for suffix in [
            ".service", ".socket", ".timer", ".scope", ".slice", ".plist",
        ] {
            if let Some(stripped) = svc_core.strip_suffix(suffix) {
                svc_core = String::from(stripped);
            }
        }
        // 模板语法取基名（getty@tty1 → getty），二进制以模板命名的
        // （agetty）不读作不匹配。
        if let Some(at) = svc_core.find('@') {
            svc_core.truncate(at);
        }
        let svc_core = svc_core.to_lowercase();
        let cmd_base = last.command.to_lowercase();
        if !svc_core.contains(&cmd_base) && !cmd_base.contains(&svc_core) {
            result.push(Warning::new(
                WarningKind::ServiceNameMismatch,
                String::from("Service name and process name do not match"),
            ));
        }
    }

    // 已删除二进制告警。
    if last.exe_deleted {
        result.push(Warning::new(
            WarningKind::DeletedBinary,
            String::from(
                "Process is running from a deleted binary (potential library injection or pending update)",
            ),
        ));
    }

    // 可疑环境变量告警（LD_PRELOAD 先于 DYLD_*，键排序保证确定性）。
    if let Some(details) = context.details {
        result.extend(env_suspicious_warnings(&details.environment));
    }
}

/// 公网监听判定（parity：`IsPublicBind`——非 LISTEN 条目被忽略，出站连接
/// 到公网地址不是暴露）。
#[must_use]
pub fn is_public_bind(sockets: &[SocketEntry]) -> bool {
    sockets
        .iter()
        .filter(|s| s.state == "LISTEN")
        .any(|s| s.address == "0.0.0.0" || s.address == "::")
}

/// 可疑工作目录名单（parity：`suspiciousDirs`）。
const SUSPICIOUS_DIRS: [&str; 3] = ["/", "/tmp", "/var/tmp"];

/// 危险 capabilities 名单（parity：`dangerousCapabilities`，逐字一致）。
const DANGEROUS_CAPABILITIES: [&str; 8] = [
    "CAP_SYS_ADMIN",
    "CAP_SYS_PTRACE",
    "CAP_NET_RAW",
    "CAP_DAC_OVERRIDE",
    "CAP_DAC_READ_SEARCH",
    "CAP_FOWNER",
    "CAP_SYS_MODULE",
    "CAP_SYS_RAWIO",
];

/// 可疑环境变量规则（parity：`envVarRules` / `envSuspiciousWarnings`）。
///
/// 两条规则固定顺序：`LD_PRELOAD`（键全等）在前、`DYLD_*`（键前缀）在后；
/// 前缀规则附带全部命中键并按 key 排序，保证多键命中时输出确定；
/// 值为空的条目不计命中（parity：`value == ""` 跳过）。
fn env_suspicious_warnings(environment: &[(String, String)]) -> Vec<Warning> {
    let mut result = Vec::new();
    if environment
        .iter()
        .any(|(key, value)| key == "LD_PRELOAD" && !value.is_empty())
    {
        result.push(Warning::new(
            WarningKind::SuspiciousEnv,
            String::from("Process sets LD_PRELOAD (potential library injection)"),
        ));
    }
    let mut dyld_keys: Vec<&str> = environment
        .iter()
        .filter(|(key, value)| key.starts_with("DYLD_") && !value.is_empty())
        .map(|(key, _)| key.as_str())
        .collect();
    if !dyld_keys.is_empty() {
        dyld_keys.sort_unstable();
        result.push(Warning::new(
            WarningKind::SuspiciousEnv,
            format!(
                "Process sets DYLD_* variables (potential library injection): {}",
                dyld_keys.join(", ")
            ),
        ));
    }
    result
}
