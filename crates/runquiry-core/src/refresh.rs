//! 自适应刷新策略与请求代际（docs/witr-parity.md §8 的纯实现）。
//!
//! 本模块只有纯函数与状态结构：不依赖 GPUI、计时器或操作系统——耗时样本由
//! 调用方测量后传入。语义对齐 witr 的 `adjustRefreshInterval`：基准 3 秒，
//! 范围 3–30 秒、步长 3 秒；连续两次超过当前间隔 60% 退避，连续两次低于 30%
//! 加速，30%–60% 区间保持（连续计数在进入中间区间时归零）。
//!
//! [`RefreshGate`] 同时承担重入门控：同一工作区在 in-flight 期间再次发起
//! （无论手工还是自动）都会被拒绝，手工刷新与自动刷新共用同一条通道。

use std::time::Duration;

/// 初始刷新间隔（对齐 top 的默认节奏）。
pub const INITIAL_INTERVAL: Duration = Duration::from_secs(3);
/// 间隔下限：退避下不降。
pub const MIN_INTERVAL: Duration = Duration::from_secs(3);
/// 间隔上限：退避上不升。
pub const MAX_INTERVAL: Duration = Duration::from_secs(30);
/// 每次退避/加速的步长。
pub const REFRESH_STEP: Duration = Duration::from_secs(3);
/// 需要连续达到多少次慢样本才退避。
pub const ADJUST_STREAK: u32 = 2;

/// 慢样本阈值分子/分母：耗时 > 间隔 × 6/10 记为慢。
const SLOW_NUM: u64 = 6;
const SLOW_DEN: u64 = 10;
/// 快样本阈值分子/分母：耗时 < 间隔 × 3/10 记为快。
const FAST_NUM: u64 = 3;
const FAST_DEN: u64 = 10;

/// 请求代际：单调递增的列表/详情请求版本号。
///
/// 工作区、目标或选择变化时调用方递增代际；携带旧代际的结果必须被丢弃。
/// 代际值本身没有语义，只有「相等才有效」这一条不变量。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Generation(u64);

impl Generation {
    /// 首个代际。
    pub const fn first() -> Self {
        Self(0)
    }

    /// 递增并返回新代际。
    #[must_use]
    pub const fn next(&mut self) -> Self {
        self.0 = self.0.wrapping_add(1);
        *self
    }

    /// 当前代际的数值（仅用于诊断输出）。
    pub const fn value(self) -> u64 {
        self.0
    }

    /// 结果是否已过期（携带者不得再把它应用到界面上）。
    pub const fn is_stale(self, current: Self) -> bool {
        self.0 != current.0
    }
}

/// 刷新门控与自适应间隔状态。
///
/// 每个工作区持有一个实例；`try_begin` 是手工与自动刷新共用的唯一入口，
/// in-flight 期间返回 `false`，从机制上杜绝重入。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RefreshGate {
    interval: Duration,
    slow_streak: u32,
    fast_streak: u32,
    in_flight: bool,
}

impl Default for RefreshGate {
    fn default() -> Self {
        Self {
            interval: INITIAL_INTERVAL,
            slow_streak: 0,
            fast_streak: 0,
            in_flight: false,
        }
    }
}

impl RefreshGate {
    /// 以初始间隔创建门控。
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前自适应间隔。
    pub const fn interval(&self) -> Duration {
        self.interval
    }

    /// 是否有刷新在进行中。
    pub const fn is_busy(&self) -> bool {
        self.in_flight
    }

    /// 尝试开始一次刷新；in-flight 期间（重入）返回 `false`。
    pub const fn try_begin(&mut self) -> bool {
        if self.in_flight {
            return false;
        }
        self.in_flight = true;
        true
    }

    /// 结束一次刷新并计入耗时样本，驱动退避/加速。
    ///
    /// 必须与 `try_begin` 成对调用；未开始的刷新不应产生样本。
    pub fn finish(&mut self, took: Duration) {
        self.in_flight = false;
        let (interval, slow, fast) =
            adjusted(self.interval, took, self.slow_streak, self.fast_streak);
        self.interval = interval;
        self.slow_streak = slow;
        self.fast_streak = fast;
    }

    /// 丢弃一次进行中的刷新（如工作区已切换）：清除 in-flight 但不产生样本，
    /// 避免把被中断的刷新当作节奏证据。
    pub const fn abort(&mut self) {
        self.in_flight = false;
    }
}

/// witr `adjustRefreshInterval` 的纯函数形式：按单个耗时样本调整间隔。
///
/// 慢/快样本各需连续 [`ADJUST_STREAK`] 次才加减一步，中间区间把两个连续
/// 计数都归零（中断即归零，不会跨非连续样本累计）。
fn adjusted(interval: Duration, took: Duration, slow: u32, fast: u32) -> (Duration, u32, u32) {
    // 用整数比较表达 60%/30% 阈值，避免浮点：took*DEN 与 interval*NUM 比较。
    let took_ms = u64::try_from(took.as_millis()).unwrap_or(u64::MAX);
    let interval_ms = u64::try_from(interval.as_millis()).unwrap_or(u64::MAX);

    if took_ms.saturating_mul(SLOW_DEN) > interval_ms.saturating_mul(SLOW_NUM) {
        let slow = slow + 1;
        if slow >= ADJUST_STREAK {
            (MAX_INTERVAL.min(interval + REFRESH_STEP), 0, 0)
        } else {
            (interval, slow, 0)
        }
    } else if took_ms.saturating_mul(FAST_DEN) < interval_ms.saturating_mul(FAST_NUM) {
        let fast = fast + 1;
        if fast >= ADJUST_STREAK {
            // saturating 下溢后由 max 拉回下限（interval 恒 ≥ MIN_INTERVAL）。
            let next = interval.saturating_sub(REFRESH_STEP).max(MIN_INTERVAL);
            (next, 0, 0)
        } else {
            (interval, 0, fast)
        }
    } else {
        (interval, 0, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration as D;

    fn gate(interval: D) -> RefreshGate {
        RefreshGate {
            interval,
            ..RefreshGate::default()
        }
    }

    /// 阈值边界：>60% 慢、<30% 快、30%–60% 保持。
    #[test]
    fn fraction_bands_classify_samples() {
        let interval = D::from_secs(10);
        // 60% 整点（6s）不慢：只有严格超过才算慢。
        assert_eq!(adjusted(interval, D::from_secs(6), 0, 0).0, interval);
        assert_eq!(adjusted(interval, D::from_millis(6_001), 0, 0).0, interval);
        assert_eq!(
            adjusted(interval, D::from_millis(6_001), 1, 0).0,
            D::from_secs(13)
        );
        // 30% 整点（3s）不快：只有严格低于才算快。
        assert_eq!(adjusted(interval, D::from_secs(3), 0, 0).0, interval);
        assert_eq!(adjusted(interval, D::from_millis(2_999), 0, 0).0, interval);
        assert_eq!(
            adjusted(interval, D::from_millis(2_999), 0, 1).0,
            D::from_secs(7)
        );
    }

    /// 连续计数：不足两次不调整；跨入中间区间立即归零（中断即归零）。
    #[test]
    fn streaks_reset_on_band_change() {
        let interval = D::from_secs(10);
        // 一次慢样本：计数 1，不调整。
        assert_eq!(adjusted(interval, D::from_secs(7), 0, 0), (interval, 1, 0));
        // 第二次慢样本：退避 +3s，计数归零。
        assert_eq!(
            adjusted(interval, D::from_secs(7), 1, 0).0,
            D::from_secs(13)
        );
        // 慢样本之后跟一次中间样本：两个计数都归零。
        assert_eq!(adjusted(interval, D::from_secs(5), 1, 0), (interval, 0, 0));
        // 一次快样本后跟一次中间样本：同样归零。
        assert_eq!(adjusted(interval, D::from_secs(5), 0, 1), (interval, 0, 0));
        // 快样本计数同理。
        assert_eq!(adjusted(interval, D::from_secs(2), 0, 0), (interval, 0, 1));
        assert_eq!(adjusted(interval, D::from_secs(2), 0, 1).0, D::from_secs(7));
    }

    /// 上限 30s、下限 3s：连续退避/加速不会越界。
    #[test]
    fn interval_is_clamped() {
        let mut g = gate(D::from_secs(29));
        g.finish(D::from_secs(29)); // 慢样本 1：只计数，不调整
        assert_eq!(g.interval(), D::from_secs(29));
        g.finish(D::from_secs(29)); // 慢样本 2：+3s 被钳到 30s
        assert_eq!(g.interval(), MAX_INTERVAL);
        g.finish(D::from_secs(29)); // 已在上限，不再上升
        assert_eq!(g.interval(), MAX_INTERVAL);

        let mut g = gate(D::from_secs(4));
        g.finish(D::ZERO);
        g.finish(D::ZERO);
        assert_eq!(g.interval(), MIN_INTERVAL);
    }

    /// 慢样本把间隔逐步推向 30s：3s 起步需要连续慢样本若干轮。
    #[test]
    fn backoff_walks_up_to_max() {
        let mut g = RefreshGate::new();
        for expected in [3u64, 3, 6, 6, 9, 9, 12] {
            assert_eq!(g.interval(), D::from_secs(expected));
            g.finish(D::from_secs(30)); // 永远是慢样本
        }
        assert_eq!(g.interval(), D::from_secs(12));
    }

    /// 重入门控：in-flight 期间拒绝再次开始；结束后可复用。
    #[test]
    fn gate_blocks_reentry() {
        let mut g = RefreshGate::new();
        assert!(g.try_begin());
        assert!(g.is_busy());
        // 重入（无论手工还是自动）被拒绝。
        assert!(!g.try_begin());
        g.finish(D::from_millis(100));
        assert!(!g.is_busy());
        assert!(g.try_begin());
        // 中止也释放通道。
        g.abort();
        assert!(!g.is_busy());
        assert!(g.try_begin());
    }

    /// 代际：单调递增，过期判定按值相等。
    #[test]
    fn generation_is_monotonic_and_staleness_is_equality() {
        let mut current = Generation::first();
        let g0 = current;
        let g1 = current.next();
        let g2 = current.next();
        assert_ne!(g0, g1);
        assert_ne!(g1, g2);
        assert!(g0.is_stale(g2));
        assert!(g2.is_stale(g0));
        assert!(!g2.is_stale(current));
    }
}
