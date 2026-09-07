//! 控制身份重读比对与进程健康标签的纯决策（只依赖 std 与 runquiry-core）。
//!
//! 本模块不得引用 sysinfo / libc / 命令执行：集成测试以 `#[path]` 在 Linux
//! 直接编译（C1 交叉编译前的可测性边界）。OS 相关模块只负责采集原始值
//! （启动时间秒数、可执行路径），判定一律经本模块。

use std::time::SystemTime;

use runquiry_core::{HealthStatus, ProcessIdentity};

/// 控制目标身份比对（Linux `controller.rs` 同语义的 macOS 侧实现）：
/// [`ProcessIdentity::same_process`] 为真，且 expected 与 current 的
/// executable 都可得并一致。executable 只做展示比对；start_time 不可验证
/// （`None`）时 `same_process` 恒为 `false`，本函数随之拒绝。
#[must_use]
pub(crate) fn same_control_target(expected: &ProcessIdentity, current: &ProcessIdentity) -> bool {
    expected.same_process(current)
        && expected.executable().is_some()
        && expected.executable() == current.executable()
}

/// sysinfo 的进程启动秒数（epoch 起）→ `SystemTime`。
///
/// 粒度即秒（平台原语如此）：**重读与发信号之间存在无法用平台原语消除的
/// PID 复用窗口**（macOS 无 pidfd；秒内复用无法由启动时间区分），已如实
/// 披露。0 视为「不可得」（正常进程不可能启动于 1970-01-01）。
#[must_use]
pub(crate) fn start_time_from_unix_seconds(seconds: u64) -> Option<SystemTime> {
    (seconds > 0).then(|| SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(seconds))
}

/// 健康标签（witr `process_darwin.go`：Z→zombie、T→stopped；否则 %cpu > 90
/// → high-cpu、RSS > 1GiB → high-mem；其余 healthy）。
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub(crate) fn health_of(
    zombie: bool,
    stopped: bool,
    cpu_percent: f32,
    rss_bytes: u64,
) -> HealthStatus {
    if zombie {
        HealthStatus::Zombie
    } else if stopped {
        HealthStatus::Stopped
    } else if cpu_percent as f64 > 90.0 {
        HealthStatus::HighCpu
    } else if rss_bytes > 1024 * 1024 * 1024 {
        HealthStatus::HighMem
    } else {
        HealthStatus::Healthy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// 合成 PID（5150 等均为合法值；`unwrap` 被 lint deny，用 `MIN` 兜底仅
    /// 服务于测试合成数据）。
    fn pid(raw: u32) -> runquiry_core::Pid {
        runquiry_core::Pid::new(raw).unwrap_or(runquiry_core::Pid::MIN)
    }

    fn identity(seconds: u64, exe: Option<&str>) -> ProcessIdentity {
        ProcessIdentity::new(
            pid(5150),
            start_time_from_unix_seconds(seconds),
            exe.map(PathBuf::from),
        )
    }

    #[test]
    fn same_control_target_requires_verifiable_start_and_same_exe() {
        let expected = identity(1_700_000_000, Some("/Users/fixture-user/fxt-app"));
        assert!(same_control_target(&expected, &expected));
        // 启动时间不可验证（None）：保守拒绝。
        let unverifiable = ProcessIdentity::new(
            pid(5150),
            None,
            Some(PathBuf::from("/Users/fixture-user/fxt-app")),
        );
        assert!(!same_control_target(&expected, &unverifiable));
        // exe 缺失或不同：拒绝。
        assert!(!same_control_target(
            &expected,
            &identity(1_700_000_000, None)
        ));
        assert!(!same_control_target(
            &expected,
            &identity(1_700_000_000, Some("/Users/fixture-user/fxt-other"))
        ));
        // 启动时间不同（PID 复用）：拒绝。
        assert!(!same_control_target(
            &expected,
            &identity(1_700_000_001, Some("/Users/fixture-user/fxt-app"))
        ));
    }

    #[test]
    fn start_time_zero_is_unavailable_and_positive_maps_to_epoch() {
        assert_eq!(start_time_from_unix_seconds(0), None);
        assert_eq!(
            start_time_from_unix_seconds(1),
            Some(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1))
        );
    }

    #[test]
    fn health_matches_witr_thresholds() {
        assert_eq!(health_of(true, false, 0.0, 0), HealthStatus::Zombie);
        assert_eq!(health_of(false, true, 0.0, 0), HealthStatus::Stopped);
        assert_eq!(health_of(false, false, 95.0, 0), HealthStatus::HighCpu);
        assert_eq!(
            health_of(false, false, 1.0, 1024 * 1024 * 1024 + 1),
            HealthStatus::HighMem
        );
        assert_eq!(
            health_of(false, false, 10.0, 1024 * 1024 * 1024),
            HealthStatus::Healthy
        );
    }
}
