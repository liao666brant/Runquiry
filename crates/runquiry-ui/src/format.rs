//! 面向用户的时间与占位符呈现。
//!
//! 锁定依赖内没有日期库（新增依赖须经守门人批准），因此这里用标准库实现
//! 最小可读格式，供确认对话框与容器表格共用，避免各处自行 `Debug` 打印
//! `SystemTime`。

use std::time::{SystemTime, UNIX_EPOCH};

/// 值不可得时统一使用的占位符。
pub const UNAVAILABLE: &str = "—";

/// 把可选文本渲染为字符串，缺失时回退 [`UNAVAILABLE`]。
pub fn format_optional(value: Option<&str>) -> String {
    value.unwrap_or(UNAVAILABLE).to_owned()
}

/// 把系统时间格式化为 UTC 的 `YYYY-MM-DD HH:MM:SS UTC`。
///
/// `None`（平台未取得）或早于 Unix 纪元的值返回 [`UNAVAILABLE`]。使用 UTC
/// 而非本地时间：没有时区数据库可用，而 UTC 可与系统工具直接对照，也不会
/// 随宿主时区配置产生歧义。
pub fn format_timestamp(time: Option<SystemTime>) -> String {
    let Some(time) = time else {
        return UNAVAILABLE.to_string();
    };
    let Ok(since_epoch) = time.duration_since(UNIX_EPOCH) else {
        return UNAVAILABLE.to_string();
    };

    let seconds = since_epoch.as_secs();
    let (year, month, day) = civil_from_days(seconds / 86_400);
    let time_of_day = seconds % 86_400;
    let (hour, minute, second) = (
        time_of_day / 3_600,
        (time_of_day % 3_600) / 60,
        time_of_day % 60,
    );

    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} UTC")
}

/// 由 Unix 纪元以来的天数求公历年月日（Howard Hinnant 的 `civil_from_days`）。
///
/// 输入按非负天数（1970 年起）调用，故省去原算法中负数分支的纪元偏移修正。
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };

    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::{UNAVAILABLE, format_optional, format_timestamp};
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn present_text_passes_through_unchanged() {
        assert_eq!(format_optional(Some("sleep")), "sleep");
    }

    #[test]
    fn absent_text_renders_placeholder() {
        assert_eq!(format_optional(None), UNAVAILABLE);
    }

    #[test]
    fn missing_time_renders_placeholder() {
        assert_eq!(format_timestamp(None), UNAVAILABLE);
    }

    #[test]
    fn pre_epoch_time_renders_placeholder() {
        let before_epoch = UNIX_EPOCH - Duration::from_secs(1);
        assert_eq!(format_timestamp(Some(before_epoch)), UNAVAILABLE);
    }

    #[test]
    fn unix_epoch_renders_its_calendar_date() {
        assert_eq!(
            format_timestamp(Some(UNIX_EPOCH)),
            "1970-01-01 00:00:00 UTC"
        );
    }

    #[test]
    fn leap_day_is_not_shifted() {
        let leap_day = UNIX_EPOCH + Duration::from_secs(1_709_164_800);
        assert_eq!(format_timestamp(Some(leap_day)), "2024-02-29 00:00:00 UTC");
    }

    #[test]
    fn time_of_day_is_rendered_from_the_same_instant() {
        let instant = UNIX_EPOCH + Duration::from_secs(1_789_025_660);
        assert_eq!(format_timestamp(Some(instant)), "2026-09-10 07:34:20 UTC");
    }
}
