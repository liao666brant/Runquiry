//! 平台测试共享基建：确定性时钟、固定 generation 与八类失败场景。
//!
//! 仅供 `tests/` 下的集成测试通过 `mod support;` 引用；不依赖真实
//! `SystemTime::now`、真实进程、真实容器 CLI 或宿主机状态。
//! 与 runquiry-core 测试 support 的时钟/generation 同语义（两 crate 的
//! 测试无法共享模块，刻意双份维护，见 tests/fixtures/README.md）。

// 测试 crate 非 lib 目标，support 模块不对外导出：unreachable_pub 不适用；
// 共享模块由各测试目标按需取用，未用项不构成告警。
#![allow(unreachable_pub)]
#![allow(dead_code)]

pub mod fakes;

use std::time::{Duration, SystemTime};

/// 合成属主进程 ID（fixture 同值约定）。
pub const FXT_PID: u32 = 4242;
/// 合成采集时刻（相对 `UNIX_EPOCH` 的毫秒数，fixture 同值约定）。
pub const CAPTURED_AT_MS: u64 = 1_700_000_000_000;

/// 确定性时钟：现在时刻由测试显式给出并显式推进，不读墙钟。
#[derive(Debug, Clone, Copy)]
pub struct FixedClock {
    now: SystemTime,
}

impl FixedClock {
    /// 以合成毫秒时间戳构造（相对 `UNIX_EPOCH`）。
    ///
    /// # Errors
    /// 毫秒值超出 `SystemTime` 可表示范围时返回错误。
    pub fn at_epoch_ms(ms: u64) -> Result<Self, String> {
        SystemTime::UNIX_EPOCH
            .checked_add(Duration::from_millis(ms))
            .map(|now| Self { now })
            .ok_or_else(|| String::from("epoch_ms 超出 SystemTime 表示范围"))
    }

    /// 当前（确定性的）时刻。
    pub const fn now(&self) -> SystemTime {
        self.now
    }

    /// 显式推进时钟；仅测试代码调用。
    pub fn advance(&mut self, step: Duration) {
        if let Some(next) = self.now.checked_add(step) {
            self.now = next;
        }
    }
}

/// 固定 generation：模拟刷新状态机的请求代数（fixture 同值约定为 7）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Generation(u64);

impl Generation {
    /// fixture 通用代数。
    pub const FIXTURE: Self = Self(7);

    /// 以显式值构造。
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// 读取代数值。
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// 八类失败场景（fixture 场景的平台侧等价物；「格式损坏」在平台侧
/// 表现为 `CommandRunner` 返回无法解析的输出）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scenario {
    /// 正常。
    Normal,
    /// 空结果。
    Empty,
    /// 部分成功：数据与诊断并存。
    Partial,
    /// 权限失败。
    PermissionDenied,
    /// 工具缺失。
    ToolMissing,
    /// 超时。
    Timeout,
    /// 格式损坏：外部命令输出不可解析。
    MalformedOutput,
}
