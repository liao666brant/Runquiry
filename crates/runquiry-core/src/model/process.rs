//! 进程身份、进程操作（含受限 `Renice`）与进程快照模型。

use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::model::ids::Pid;

/// 进程身份：破坏性操作前重读比对，防止 PID 被复用导致误操作。
///
/// 身份判定只看 PID 与启动时间（parity：`pidIdentityChanged` 比较 PID + `StartedAt`）；
/// `executable` 是展示信息，不参与判定。本类型刻意不实现 `PartialEq`：
/// `start_time` 为 `None` 表示平台未能取得启动时间，此时身份不可验证，
/// 值比较会得出「未变化」的假阴性。一切身份判定必须走
/// [`ProcessIdentity::same_process`]，由它对 `None` 保守返回「不同身份」。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessIdentity {
    /// 进程 ID。
    pid: Pid,
    /// 进程启动时间；平台无法取得时为 `None`（此时无法证明身份未变）。
    start_time: Option<SystemTime>,
    /// 可执行文件路径。
    executable: Option<PathBuf>,
}

impl ProcessIdentity {
    /// 构造进程身份。
    pub const fn new(
        pid: Pid,
        start_time: Option<SystemTime>,
        executable: Option<PathBuf>,
    ) -> Self {
        Self {
            pid,
            start_time,
            executable,
        }
    }

    /// 读取进程 ID。
    pub const fn pid(&self) -> Pid {
        self.pid
    }

    /// 读取启动时间。
    pub const fn start_time(&self) -> Option<SystemTime> {
        self.start_time
    }

    /// 读取可执行文件路径。
    pub const fn executable(&self) -> Option<&PathBuf> {
        self.executable.as_ref()
    }

    /// 判定两个快照是否属于同一个可验证的进程。
    ///
    /// 仅当 PID 相同、启动时间相同且启动时间可得（`Some`）时返回 `true`。
    /// 任一侧 `start_time` 为 `None` 即视为不同身份：无法证明身份未变时，
    /// 必须保守拒绝破坏性操作，而不是假定进程未被复用。
    pub fn same_process(&self, other: &Self) -> bool {
        self.pid == other.pid && self.start_time.is_some() && self.start_time == other.start_time
    }
}

/// 进程操作类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessAction {
    /// 温和终止（SIGTERM）。
    Terminate,
    /// 强制终止（SIGKILL）。
    Kill,
    /// 暂停（SIGSTOP）。
    Pause,
    /// 恢复（SIGCONT）。
    Resume,
    /// 调整调度优先级；值被 [`Renice`] 新类型限制在 -20..=19。
    Renice(Renice),
}

/// 调度优先级调整值，仅允许 -20..=19（parity：越界在调用 setpriority 前拒绝）。
///
/// 字段私有且 `Deserialize` 带校验，因此越界值在任何路径下都不可能被构造。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Renice(i8);

impl Renice {
    /// 允许的最小值。
    pub const MIN_VALUE: i8 = -20;
    /// 允许的最大值。
    pub const MAX_VALUE: i8 = 19;

    /// 读取原始数值。
    pub const fn get(self) -> i8 {
        self.0
    }
}

/// `Renice` 取值越界错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReniceOutOfRange {
    /// 被拒绝的原始值。
    value: i8,
}

impl std::fmt::Display for ReniceOutOfRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "renice 取值 {} 越界，允许范围 {}..={}",
            self.value,
            Renice::MIN_VALUE,
            Renice::MAX_VALUE
        )
    }
}

impl std::error::Error for ReniceOutOfRange {}

impl TryFrom<i8> for Renice {
    type Error = ReniceOutOfRange;

    fn try_from(value: i8) -> Result<Self, Self::Error> {
        if (Self::MIN_VALUE..=Self::MAX_VALUE).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ReniceOutOfRange { value })
        }
    }
}

impl Serialize for Renice {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i8(self.0)
    }
}

impl<'de> Deserialize<'de> for Renice {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = i8::deserialize(deserializer)?;
        Self::try_from(value).map_err(serde::de::Error::custom)
    }
}

/// 进程列表条目：进程基线字段（parity：PID、PPID、命令名、完整命令行、启动时间、用户）。
///
/// 不实现 `PartialEq`：条目内含 [`ProcessIdentity`]，而身份判定必须经由
/// [`ProcessIdentity::same_process`]（`start_time` 为 `None` 时不可验证），
/// 逐字段值比较会掩盖 PID 复用。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessSummary {
    /// 进程身份。
    pub identity: ProcessIdentity,
    /// 父进程 ID；不可得时为 `None`。
    pub parent_pid: Option<Pid>,
    /// 命令名（comm / 可执行名）。
    pub command: String,
    /// 完整命令行；不可得时为 `None`。
    pub command_line: Option<String>,
    /// 进程属主用户名。
    pub user: Option<String>,
}

/// 进程详情快照：资源、工作目录、环境变量与子进程。
///
/// 不实现 `PartialEq`，理由同 [`ProcessSummary`]：身份判定必须经由
/// [`ProcessIdentity::same_process`]。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessDetails {
    /// 详情对应的进程身份。
    pub identity: ProcessIdentity,
    /// CPU 使用率百分比（首样本前为 `None`，UI 显示「采样中」）。
    pub cpu_percent: Option<f64>,
    /// 常驻内存字节数。
    pub memory_rss_bytes: Option<u64>,
    /// 内存占系统总量百分比。
    pub memory_percent: Option<f64>,
    /// 工作目录。
    pub working_dir: Option<PathBuf>,
    /// 环境变量（key=value），UI 侧默认脱敏。
    pub environment: Vec<(String, String)>,
    /// 子进程 PID 列表（按 PID 升序）。
    pub children: Vec<Pid>,
}
