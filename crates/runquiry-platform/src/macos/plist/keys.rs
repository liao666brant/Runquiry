//! plist 事件流 → launchd 关键字段提取与人读文本（witr 状态机同语义）。

use super::reader::{Event, tokenize};

/// launchd plist 关键字段的提取结果（witr `LaunchdInfo` 的 plist 来源子集）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LaunchdPlistInfo {
    /// `Label`。
    pub label: String,
    /// `Comment`（可选描述）。
    pub comment: String,
    /// `Program`（可选，单程序路径）。
    pub program: String,
    /// `ProgramArguments`。
    pub program_arguments: Vec<String>,
    /// `RunAtLoad`。
    pub run_at_load: bool,
    /// `KeepAlive`（bool 形式；dict 形式按 false，witr 同语义）。
    pub keep_alive: bool,
    /// `StartInterval`（秒；未设为 0）。
    pub start_interval: i64,
    /// `StartCalendarInterval` 的人读文本（witr `formatCalendarInterval`）。
    pub start_calendar_interval: String,
    /// `WatchPaths`。
    pub watch_paths: Vec<String>,
    /// `QueueDirectories`。
    pub queue_directories: Vec<String>,
}

/// 触发器人读文本（witr `FormatTriggers`）。
#[must_use]
pub(crate) fn format_triggers(info: &LaunchdPlistInfo) -> Vec<String> {
    let mut triggers = Vec::new();
    if info.run_at_load {
        triggers.push(String::from("RunAtLoad (starts at login/boot)"));
    }
    if info.start_interval > 0 {
        triggers.push(format!(
            "StartInterval (every {})",
            format_duration(info.start_interval)
        ));
    }
    if !info.start_calendar_interval.is_empty() {
        triggers.push(format!(
            "StartCalendarInterval ({})",
            info.start_calendar_interval
        ));
    }
    for path in &info.watch_paths {
        triggers.push(format!("WatchPaths: {path}"));
    }
    for path in &info.queue_directories {
        triggers.push(format!("QueueDirectories: {path}"));
    }
    triggers
}

/// 解析 XML plist 文本（`plutil -convert xml1 -o -` 的输出）。
///
/// 损坏输入按「能解析到多少就返回多少」处理（witr 解码器遇错即停的同语义，
/// 调用方对空结果记诊断）。
#[must_use]
pub(crate) fn parse_plist_xml(data: &str) -> LaunchdPlistInfo {
    let events = tokenize(data);
    let mut info = LaunchdPlistInfo::default();
    apply_events(&events, &mut info);
    info
}

/// 事件流 → 字段（witr `parsePlistXML` 的 dictDepth / currentKey 状态机）。
fn apply_events(events: &[Event], info: &mut LaunchdPlistInfo) {
    let mut dict_depth: usize = 0;
    let mut current_key = String::new();
    let mut idx = 0;
    while idx < events.len() {
        match &events[idx] {
            Event::Dict => {
                dict_depth += 1;
                if dict_depth == 2 && current_key == "StartCalendarInterval" {
                    info.start_calendar_interval = parse_calendar_dict(&events[idx + 1..]);
                    current_key.clear();
                } else if dict_depth > 1 {
                    current_key.clear();
                }
            }
            Event::EndDict => dict_depth = dict_depth.saturating_sub(1),
            Event::Array if dict_depth == 1 => {
                if current_key == "StartCalendarInterval" {
                    info.start_calendar_interval = parse_calendar_array(&events[idx + 1..]);
                } else {
                    let values = string_array(&events[idx + 1..]);
                    assign_array(info, &current_key, values);
                }
                current_key.clear();
            }
            Event::Key(key) if dict_depth == 1 => current_key = key.clone(),
            Event::Str(value) if dict_depth == 1 && !current_key.is_empty() => {
                assign_string(info, &current_key, value.clone());
                current_key.clear();
            }
            Event::Int(raw) if dict_depth == 1 && !current_key.is_empty() => {
                if current_key == "StartInterval"
                    && let Ok(parsed) = raw.trim().parse::<i64>()
                {
                    info.start_interval = parsed;
                }
                current_key.clear();
            }
            Event::True if dict_depth == 1 && !current_key.is_empty() => {
                assign_bool(info, &current_key, true);
                current_key.clear();
            }
            Event::False if dict_depth == 1 && !current_key.is_empty() => {
                assign_bool(info, &current_key, false);
                current_key.clear();
            }
            _ => {}
        }
        idx += 1;
    }
}

fn assign_string(info: &mut LaunchdPlistInfo, key: &str, value: String) {
    match key {
        "Label" => info.label = value,
        "Comment" => info.comment = value,
        "Program" => info.program = value,
        _ => {}
    }
}

fn assign_bool(info: &mut LaunchdPlistInfo, key: &str, value: bool) {
    match key {
        "RunAtLoad" => info.run_at_load = value,
        "KeepAlive" => info.keep_alive = value,
        _ => {}
    }
}

fn assign_array(info: &mut LaunchdPlistInfo, key: &str, values: Vec<String>) {
    match key {
        "ProgramArguments" => info.program_arguments = values,
        "WatchPaths" => info.watch_paths = values,
        "QueueDirectories" => info.queue_directories = values,
        _ => {}
    }
}

/// 事件流（直到配平的 `</dict>`）→ 日历 dict 人读文本（witr
/// `parseCalendarDict` + `formatCalendarInterval`）。
fn parse_calendar_dict(events: &[Event]) -> String {
    let mut fields: Vec<(String, i64)> = Vec::new();
    let mut depth: usize = 0;
    let mut key = String::new();
    for event in events {
        match event {
            Event::Dict => depth += 1,
            Event::EndDict => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            Event::Key(name) if depth == 0 => key = name.clone(),
            Event::Int(raw) if depth == 0 && !key.is_empty() => {
                if let Ok(value) = raw.trim().parse::<i64>() {
                    fields.push((key.clone(), value));
                }
                key.clear();
            }
            _ => {}
        }
    }
    format_calendar_interval(&fields)
}

/// 事件流（直到配平的 `</array>`）→ 日历数组人读文本（witr
/// `parseCalendarArray`：多个日历 dict 以 `; ` 连接）。
fn parse_calendar_array(events: &[Event]) -> String {
    let mut intervals = Vec::new();
    let mut depth: usize = 1;
    let mut start: Option<usize> = None;
    for (idx, event) in events.iter().enumerate() {
        match event {
            Event::Dict => {
                if depth == 1 {
                    start = Some(idx + 1);
                }
                depth += 1;
            }
            Event::EndDict => {
                if depth == 2
                    && let Some(from) = start.take()
                {
                    let text = parse_calendar_dict(&events[from..idx]);
                    if !text.is_empty() {
                        intervals.push(text);
                    }
                }
                depth = depth.saturating_sub(1);
            }
            Event::EndArray => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Event::Array => depth += 1,
            _ => {}
        }
    }
    intervals.join("; ")
}

/// 数组事件流（直到配平的 `</array>`）→ 字符串列表（witr `parseArray`）。
fn string_array(events: &[Event]) -> Vec<String> {
    let mut values = Vec::new();
    let mut depth: usize = 1;
    for event in events {
        match event {
            Event::Array => depth += 1,
            Event::EndArray => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Event::Str(value) if depth == 1 => values.push(value.clone()),
            _ => {}
        }
    }
    values
}

/// 日历 dict → 人读文本（witr `formatCalendarInterval`）：
/// `Weekday month day at HH:MM`。键：Month / Day / Weekday（0=Sun）/
/// Hour / Minute。
fn format_calendar_interval(fields: &[(String, i64)]) -> String {
    if fields.is_empty() {
        return String::new();
    }
    let weekdays = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let value_of = |key: &str| fields.iter().find(|(k, _)| k == key).map(|(_, v)| *v);
    let mut parts: Vec<String> = Vec::new();
    if let Some(weekday) = value_of("Weekday")
        && (0..weekdays.len() as i64).contains(&weekday)
    {
        let index = usize::try_from(weekday).unwrap_or(0);
        parts.push(String::from(weekdays[index]));
    }
    if let Some(month) = value_of("Month") {
        parts.push(format!("month {month}"));
    }
    if let Some(day) = value_of("Day") {
        parts.push(format!("day {day}"));
    }
    match (value_of("Hour"), value_of("Minute")) {
        (Some(hour), Some(minute)) => parts.push(format!("at {hour:02}:{minute:02}")),
        (Some(hour), None) => parts.push(format!("at {hour:02}:00")),
        (None, Some(minute)) => parts.push(format!("at *:{minute:02}")),
        (None, None) => {}
    }
    parts.join(" ")
}

/// 秒数 → 人读时长（witr `formatDuration`：s/m/h/d 单位取舍）。
fn format_duration(seconds: i64) -> String {
    if seconds < 60 {
        return format!("{seconds}s");
    }
    if seconds < 3600 {
        return format!("{}m", seconds / 60);
    }
    if seconds < 86_400 {
        return format!("{}h", seconds / 3600);
    }
    format!("{}d", seconds / 86_400)
}
