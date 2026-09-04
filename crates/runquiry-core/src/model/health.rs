//! 进程健康状态标签（parity §1 `Process.Health`）。

use serde::{Deserialize, Serialize};

/// 进程健康状态（parity：healthy / zombie / stopped / high-cpu / high-mem 五种
/// 标签，Runquiry 增加 `Unknown` 表示未采集或不可判定）。
///
/// 序列化值与 witr 标签字符串逐字一致（`high-cpu` 为连字符形式），供 fixture
/// 与告警分支使用；`Default` 为 [`HealthStatus::Unknown`]，配合
/// `#[serde(default)]` 保证旧序列化条目可读。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HealthStatus {
    /// 未知 / 未采集；`#[serde(default)]` 兜底。
    #[default]
    #[serde(rename = "unknown")]
    Unknown,
    /// 正常运行。
    #[serde(rename = "healthy")]
    Healthy,
    /// 僵尸进程。
    #[serde(rename = "zombie")]
    Zombie,
    /// 已停止（如 SIGSTOP 暂停）。
    #[serde(rename = "stopped")]
    Stopped,
    /// 高 CPU 占用（witr 告警阈值：累计 CPU 时间 > 2h）。
    #[serde(rename = "high-cpu")]
    HighCpu,
    /// 高内存占用（witr 告警阈值：RSS > 1GB）。
    #[serde(rename = "high-mem")]
    HighMem,
}

impl HealthStatus {
    /// witr 标签字符串（与序列化值一致）。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Healthy => "healthy",
            Self::Zombie => "zombie",
            Self::Stopped => "stopped",
            Self::HighCpu => "high-cpu",
            Self::HighMem => "high-mem",
        }
    }
}
