//! 容器机器输出的共享解析助手：健康提取、容器 ID 校验与时间解析。
//!
//! 时间格式对齐 witr 证据（`runtime_dockerlike.go` 的 `parseDockerTime`）：
//! RFC3339(Nano) 与 docker `CreatedAt` 的 `2006-01-02 15:04:05 -0700 MST`
//! 家族；无时区偏移时按 UTC 处理。不引入时间库，纯手写解析。

// 容器子模块为私有模块，pub(crate) 是父模块可见的最小可见性（仓库约定豁免）。
#![allow(clippy::redundant_pub_crate)]

use std::time::{Duration, SystemTime};

/// 从状态文本尾部提取健康检查结果（parity：`healthFromStatus`）。
///
/// `Up 4 minutes (healthy)` → `healthy`；括注不在白名单
/// （healthy / unhealthy / starting / `health: starting`）时返回 `None`。
pub(crate) fn health_from_status(status: &str) -> Option<String> {
    let trimmed = status.trim_end();
    if !trimmed.ends_with(')') {
        return None;
    }
    let open = trimmed.rfind('(')?;
    let inner = trimmed[open + 1..trimmed.len() - 1].trim();
    let lowered = inner.to_ascii_lowercase();
    let value = match lowered.as_str() {
        "healthy" => "healthy",
        "unhealthy" => "unhealthy",
        "health: starting" | "starting" => "starting",
        _ => return None,
    };
    Some(value.to_string())
}

/// 容器 ID 是否可以安全交给运行时 CLI（parity：`isValidContainerID`）。
///
/// 非空、仅含 `[A-Za-z0-9_.-]` 且以字母数字开头；拒绝前导 `-`，防止 cgroup
/// 解析出的脏 ID 被当作 CLI 选项。
pub(crate) fn is_valid_container_id(id: &str) -> bool {
    !id.is_empty()
        && id.bytes().enumerate().all(|(idx, byte)| {
            matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9')
                || (idx > 0 && matches!(byte, b'_' | b'.' | b'-'))
        })
}

/// 解析容器运行时输出的时间戳（RFC3339(Nano) 或 docker `CreatedAt` 家族）。
///
/// 返回 `None` 的情形：格式无法识别、日期越界或早于 UNIX epoch（docker 的
/// 零值 `0001-01-01T00:00:00Z` 由此自然被过滤）。
pub(crate) fn parse_machine_time(raw: &str) -> Option<SystemTime> {
    let text = raw.trim();
    if text.is_empty() {
        return None;
    }
    let (date_part, rest) = text.split_once(['T', ' '])?;
    let (year, month, day) = parse_date(date_part)?;
    let (hour, minute, second, nanos, offset_secs) = parse_time_and_offset(rest)?;
    let days = days_from_civil(year, month, day);
    let epoch_days_secs = days.checked_mul(86_400)?;
    let epoch = epoch_days_secs
        .checked_add(i64::from(hour) * 3_600 + i64::from(minute) * 60 + i64::from(second))?
        .checked_sub(offset_secs)?;
    if epoch.is_negative() {
        return None;
    }
    let secs = u64::try_from(epoch).ok()?;
    SystemTime::UNIX_EPOCH.checked_add(Duration::new(secs, nanos))
}

/// 解析 `YYYY-MM-DD`（严格两位月/日、范围校验）。
fn parse_date(part: &str) -> Option<(i64, u32, u32)> {
    let segments: Vec<&str> = part.split('-').collect();
    if segments.len() != 3
        || segments[0].len() != 4
        || segments[1].len() != 2
        || segments[2].len() != 2
    {
        return None;
    }
    let year: i64 = segments[0].parse().ok()?;
    let month: u32 = segments[1].parse().ok()?;
    let day: u32 = segments[2].parse().ok()?;
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return None;
    }
    Some((year, month, day))
}

/// 解析 `HH:MM[:SS][.frac]` 及其后的时区偏移（`Z` / `±HH:MM` / `±HHMM`，可
/// 带尾部时区名；无偏移按 UTC）。秒缺省为 0（docker `CreatedAt` 家族总是带秒）。
fn parse_time_and_offset(rest: &str) -> Option<(u32, u32, u32, u32, i64)> {
    let rest = rest.trim();
    let (time_part, tail) = rest
        .find(['+', '-'])
        .map_or((rest, ""), |idx| rest.split_at(idx));
    // RFC3339 的 `Z` 后缀紧贴秒字段（无偏移符号），先剥离；docker 格式的
    // 偏移前有空格，一并修剪。
    let time_part = time_part.trim_end();
    let time_part = time_part
        .strip_suffix('z')
        .or_else(|| time_part.strip_suffix('Z'))
        .unwrap_or(time_part);
    let (hms, frac) = match time_part.split_once('.') {
        Some((h, f)) => (h, f),
        None => (time_part, ""),
    };
    let mut fields = hms.split(':');
    let hour: u32 = fields.next()?.parse().ok()?;
    let minute: u32 = fields.next()?.parse().ok()?;
    let second: u32 = match fields.next() {
        Some(value) => value.parse().ok()?,
        None => 0,
    };
    if fields.next().is_some() || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let nanos = parse_fraction(frac)?;
    let offset = parse_offset(tail.trim())?;
    Some((hour, minute, second, nanos, offset))
}

/// 小数秒 → 纳秒（超出 9 位截断，不足右补零）。
fn parse_fraction(frac: &str) -> Option<u32> {
    if frac.is_empty() {
        return Some(0);
    }
    let digits: String = frac.chars().take(9).filter(char::is_ascii_digit).collect();
    if digits.len() != frac.len().min(9) {
        return None;
    }
    let padded = format!("{digits:0<9}");
    padded.parse().ok()
}

/// 解析时区偏移：`Z`/空 → 0；`±HH:MM` / `±HHMM` → 秒数（西半球为负）。
fn parse_offset(tail: &str) -> Option<i64> {
    if tail.is_empty() || tail.eq_ignore_ascii_case("z") {
        return Some(0);
    }
    let (sign, body) = match tail.as_bytes()[0] {
        b'+' => (1_i64, &tail[1..]),
        b'-' => (-1_i64, &tail[1..]),
        _ => return None,
    };
    let body = body.split(' ').next()?;
    if !body.chars().all(|c| c.is_ascii_digit() || c == ':') {
        return None;
    }
    let digits: String = body.chars().filter(char::is_ascii_digit).collect();
    let (hours, minutes) = match digits.len() {
        2 => (digits.parse::<i64>().ok()?, 0),
        4 => (digits[..2].parse().ok()?, digits[2..].parse().ok()?),
        _ => return None,
    };
    if hours > 23 || minutes > 59 {
        return None;
    }
    Some(sign * (hours * 3_600 + minutes * 60))
}

/// 平年/闰年各月天数。
const fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

const fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Hinnant `days_from_civil`：公历日期 → 自 1970-01-01 起的天数。
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_pos = i64::from((month + 9) % 12);
    let day_of_year = (153 * month_pos + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_from_status_extracts_whitelisted_values() {
        assert_eq!(
            health_from_status("Up 4 minutes (healthy)").as_deref(),
            Some("healthy")
        );
        assert_eq!(
            health_from_status("Up 2 hours (health: starting)").as_deref(),
            Some("starting")
        );
        assert_eq!(
            health_from_status("Up 1 minute (unhealthy)  ").as_deref(),
            Some("unhealthy")
        );
        assert_eq!(
            health_from_status("Up 4 minutes (starting)").as_deref(),
            Some("starting")
        );
        assert_eq!(health_from_status("Up 4 minutes (restarting)"), None);
        assert_eq!(health_from_status("Up 4 minutes"), None);
        assert_eq!(health_from_status(""), None);
    }

    #[test]
    fn valid_container_id_rejects_unsafe_tokens() {
        assert!(is_valid_container_id("5f2d4a1b9c8e"));
        assert!(is_valid_container_id("web-01.payload"));
        assert!(!is_valid_container_id(""));
        assert!(!is_valid_container_id("-leading"));
        assert!(!is_valid_container_id(".leading"));
        assert!(!is_valid_container_id("has space"));
        assert!(!is_valid_container_id("has/slash"));
    }

    #[test]
    fn machine_time_parses_rfc3339_and_docker_datetime() {
        let epoch = std::time::UNIX_EPOCH;
        let parsed = parse_machine_time("2026-08-30T10:00:00Z");
        assert_eq!(
            parsed
                .and_then(|t| t.duration_since(epoch).ok())
                .map(|d| d.as_secs()),
            Some(1_788_084_000)
        );
        // docker CreatedAt 家族（显式偏移 + 时区名）与 RFC3339 等价。
        assert_eq!(parse_machine_time("2026-08-30 10:00:00 +0000 UTC"), parsed);
        assert_eq!(parse_machine_time("2026-08-30 18:00:00 +0800 CST"), parsed);
        assert!(parse_machine_time("2026-08-30T10:00:00.123456789+08:00").is_some());
    }

    #[test]
    fn machine_time_rejects_unparsable_and_pre_epoch_values() {
        assert_eq!(parse_machine_time(""), None);
        assert_eq!(parse_machine_time("not a time"), None);
        assert_eq!(parse_machine_time("2026-13-01T00:00:00Z"), None);
        assert_eq!(parse_machine_time("2026-02-30T00:00:00Z"), None);
        // docker 零值时间早于 epoch，自然被过滤（witr enrich 的跳过语义）。
        assert_eq!(parse_machine_time("0001-01-01T00:00:00Z"), None);
    }
}
