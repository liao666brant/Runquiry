//! A5 测试共享基建：确定性时钟、固定 generation、fixture 装载与契约级假后端。
//!
//! 仅供 `tests/` 下的集成测试通过 `mod support;` 引用；不进入公共 API，
//! 不依赖真实 `SystemTime::now`、真实进程或宿主机状态。

// 测试 crate 非 lib 目标，support 模块不对外导出：unreachable_pub 不适用；
// 共享模块由各测试目标按需取用，未用项不构成告警。
#![allow(unreachable_pub)]
#![allow(dead_code)]

pub mod collectors;
pub mod fakes;
pub mod fixtures;

use std::time::{Duration, SystemTime};

/// 确定性时钟：现在时刻由测试显式给出并显式推进，不读墙钟。
///
/// 假后端经 [`runquiry_core::Inspection::with_captured_at`] 消费
/// [`FixedClock::now`]，使快照时刻在测试间可复现。
#[derive(Debug, Clone, Copy)]
pub struct FixedClock {
    now: SystemTime,
}

impl FixedClock {
    /// 以合成毫秒时间戳构造（相对 `UNIX_EPOCH`，与 fixture 的
    /// `captured_at_epoch_ms` 同一取值约定）。
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

/// 固定 generation：模拟刷新状态机的请求代数，取值由测试显式给定。
///
/// 测试统一使用 `Generation::FIXTURE`（7），与 fixture 封套的 `generation`
/// 字段一致，断言「数据属于哪一代请求」不依赖任何全局可变状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Generation(u64);

impl Generation {
    /// fixture 通用代数（与 `tests/fixtures/` 各封套一致）。
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
