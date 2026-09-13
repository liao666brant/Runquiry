//! 进程 CPU% 两样本差分（parity line 67 intentional change）。
//!
//! 平台采集器只提供进程生命周期累计 CPU 秒（[`ProcessSummary::
//! cpu_time_seconds`]）；本模块在装配层跨刷新保留上次样本，把相邻两次
//! 快照的 CPU 时间差除以墙钟间隔得到使用率百分比，写回
//! [`ProcessSummary::cpu_percent`]。首样本、墙钟间隔非正或 CPU 时间倒退
//! （PID 复用）时 `cpu_percent` 为 `None`。

use std::collections::HashMap;
use std::time::Instant;

use runquiry_core::{Pid, ProcessSummary};

/// 跨刷新保留的 CPU 采样状态。
#[derive(Debug, Default)]
pub(super) struct CpuSampleStore {
    /// pid → 上次快照的累计 CPU 秒。
    previous: HashMap<Pid, f64>,
    /// 上次 `apply` 时刻（墙钟间隔基准）。
    last_refresh: Option<Instant>,
}

/// 由前后两样本的累计 CPU 秒与墙钟间隔计算 CPU 使用率百分比。
///
/// 墙钟间隔非正（含 NaN）或 CPU 时间倒退（PID 复用/计数器异常，含 NaN）
/// 返回 `None`，调用方按「本次无样本」处理。
fn cpu_percent_between(previous_secs: f64, current_secs: f64, wall_secs: f64) -> Option<f64> {
    if wall_secs > 0.0 && current_secs >= previous_secs {
        Some((current_secs - previous_secs) / wall_secs * 100.0)
    } else {
        None
    }
}

impl CpuSampleStore {
    /// 就地写回本轮 `cpu_percent`，并把本轮样本记录为下一次的基准。
    ///
    /// 死亡进程的样本不保留：`previous` 每轮整体替换为本轮集合，避免
    /// 长驻增长与 PID 复用后的陈旧基准。
    pub(super) fn apply(&mut self, summaries: &mut [ProcessSummary]) {
        let now = Instant::now();
        let wall_secs = self
            .last_refresh
            .map_or(0.0, |last| last.elapsed().as_secs_f64());
        let previous = std::mem::take(&mut self.previous);
        let mut current = HashMap::with_capacity(summaries.len());
        for summary in summaries {
            let Some(secs) = summary.cpu_time_seconds else {
                continue;
            };
            let pid = summary.identity.pid();
            summary.cpu_percent = previous
                .get(&pid)
                .and_then(|prev| cpu_percent_between(*prev, secs, wall_secs));
            current.insert(pid, secs);
        }
        self.previous = current;
        self.last_refresh = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::cpu_percent_between;

    #[test]
    fn percent_is_delta_cpu_over_wall_time() {
        // 0.5 秒 CPU / 2 秒墙钟 = 25%。
        let percent = cpu_percent_between(10.0, 10.5, 2.0);

        assert!((percent.unwrap_or(0.0) - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn first_sample_without_wall_time_is_none() {
        assert_eq!(cpu_percent_between(0.0, 10.0, 0.0), None);
    }

    #[test]
    fn backwards_cpu_time_is_treated_as_no_sample() {
        // PID 复用：新进程的累计 CPU 秒小于旧样本。
        assert_eq!(cpu_percent_between(100.0, 1.0, 2.0), None);
    }

    #[test]
    fn unchanged_cpu_time_reports_zero_percent() {
        let percent = cpu_percent_between(10.0, 10.0, 2.0);

        assert_eq!(percent, Some(0.0));
    }
}
