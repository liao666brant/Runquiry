//! systemd D-Bus 属性读取与 timer schedule 解析。

use std::collections::HashMap;
use std::time::Duration;

use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

const DBUS_TIMEOUT: Duration = Duration::from_secs(2);
const SYSTEMD_DESTINATION: &str = "org.freedesktop.systemd1";
const SYSTEMD_OBJECT_PATH: &str = "/org/freedesktop/systemd1";
const SYSTEMD_MANAGER: &str = "org.freedesktop.systemd1.Manager";
const PROPERTIES_INTERFACE: &str = "org.freedesktop.DBus.Properties";

/// systemd D-Bus 富化（parity：`enrichFromSystemd` 的键——`Description` /
/// `FragmentPath` / `SourcePath` / `NRestarts` / 定时器 `schedule`）。
pub(super) fn enrich_systemd(unit: &str) -> Vec<(String, String)> {
    let Ok(builder) = zbus::blocking::connection::Builder::system() else {
        return Vec::new();
    };
    let Ok(connection) = builder.method_timeout(DBUS_TIMEOUT).build() else {
        return Vec::new();
    };
    let Ok(manager) = zbus::blocking::Proxy::new(
        &connection,
        SYSTEMD_DESTINATION,
        SYSTEMD_OBJECT_PATH,
        SYSTEMD_MANAGER,
    ) else {
        return Vec::new();
    };
    let Ok(unit_path) = manager.call::<_, _, OwnedObjectPath>("GetUnit", &(unit,)) else {
        return Vec::new();
    };
    let mut details = Vec::new();
    if let Some(properties) = get_all(&connection, &unit_path, "org.freedesktop.systemd1.Unit") {
        append_unit_properties(&mut details, &properties);
    }
    if let Some(base) = unit.strip_suffix(".service") {
        if let Some(service) = get_all(&connection, &unit_path, "org.freedesktop.systemd1.Service")
            && let Some(restarts) = u32_prop(&service, "NRestarts")
        {
            details.push((String::from("NRestarts"), restarts.to_string()));
        }
        let timer_unit = format!("{base}.timer");
        if let Ok(timer_path) =
            manager.call::<_, _, OwnedObjectPath>("GetUnit", &(timer_unit.as_str(),))
            && let Some(timer) = get_all(&connection, &timer_path, "org.freedesktop.systemd1.Timer")
            && let Some(schedule) = timer_schedule(&timer)
        {
            details.push((String::from("schedule"), schedule));
        }
    }
    details
}

/// Unit 接口属性 → 富化键值（顺序固定，core 只透传）。
fn append_unit_properties(
    details: &mut Vec<(String, String)>,
    properties: &HashMap<String, OwnedValue>,
) {
    if let Some(description) = string_prop(properties, "Description") {
        details.push((String::from("Description"), description));
    }
    if let Some(fragment) = string_prop(properties, "FragmentPath").filter(|path| !path.is_empty())
    {
        details.push((String::from("FragmentPath"), fragment));
    } else if let Some(source) =
        string_prop(properties, "SourcePath").filter(|path| !path.is_empty())
    {
        details.push((String::from("SourcePath"), source));
    }
}

/// `Properties.GetAll` 调用（失败返回 `None`，调用方按 best-effort 省略）。
fn get_all(
    connection: &zbus::blocking::Connection,
    path: &OwnedObjectPath,
    interface: &str,
) -> Option<HashMap<String, OwnedValue>> {
    let proxy =
        zbus::blocking::Proxy::new(connection, SYSTEMD_DESTINATION, path, PROPERTIES_INTERFACE)
            .ok()?;
    proxy.call("GetAll", &(interface,)).ok()
}

fn string_prop(properties: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    let value = properties.get(key)?;
    match &**value {
        Value::Str(text) => Some(text.as_str().to_string()),
        Value::Value(inner) => match &**inner {
            Value::Str(text) => Some(text.as_str().to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn u32_prop(properties: &HashMap<String, OwnedValue>, key: &str) -> Option<u32> {
    let value = properties.get(key)?;
    match &**value {
        Value::U32(number) => Some(*number),
        Value::Value(inner) => match &**inner {
            Value::U32(number) => Some(*number),
            _ => None,
        },
        _ => None,
    }
}

/// 递归取字符串（variant 包装时穿透一层）。
fn as_string(value: &Value<'_>) -> Option<String> {
    match value {
        Value::Str(text) => Some(text.as_str().to_string()),
        Value::Value(inner) => as_string(inner),
        _ => None,
    }
}

/// 递归取 u64（variant 包装时穿透一层）。
fn as_u64(value: &Value<'_>) -> Option<u64> {
    match value {
        Value::U64(number) => Some(*number),
        Value::Value(inner) => as_u64(inner),
        _ => None,
    }
}

/// 定时器调度（parity：`timerSchedule`；相对时间格式化属展示层，这里只取
/// 调度表达式：calendar spec 或 monotonic 的 "every …" 短语）。
fn timer_schedule(properties: &HashMap<String, OwnedValue>) -> Option<String> {
    if let Some(spec) = properties
        .get("TimersCalendar")
        .and_then(|value| calendar_spec(value))
    {
        return Some(spec);
    }
    if let Some(value) = properties.get("TimersMonotonic") {
        return monotonic_spec(value);
    }
    None
}

/// TimersCalendar（数组，每项含日历表达式字符串）→ 表达式
/// （parity：`calendarSpec` 取每项的 spec 字符串字段）。
fn calendar_spec(value: &Value<'_>) -> Option<String> {
    let Value::Array(items) = value else {
        return None;
    };
    for item in items.iter() {
        let Value::Structure(fields) = item else {
            continue;
        };
        for field in fields.fields() {
            if let Value::Str(spec) = field
                && !spec.as_str().is_empty()
            {
                return Some(spec.as_str().to_string());
            }
        }
    }
    None
}

/// TimersMonotonic（数组，每项形如 (base, usec, …)）→ "every …" 短语
/// （parity：`monotonicSpec` 的三种前缀分支）。
fn monotonic_spec(value: &Value<'_>) -> Option<String> {
    let Value::Array(items) = value else {
        return None;
    };
    for item in items.iter() {
        let Value::Structure(fields) = item else {
            continue;
        };
        let all = fields.fields();
        let Some(base) = all.first().and_then(as_string) else {
            continue;
        };
        let Some(usec) = all.get(1).and_then(as_u64) else {
            continue;
        };
        if usec == 0 {
            continue;
        }
        let human = human_duration(usec);
        return Some(if base.starts_with("OnBoot") {
            format!("every boot + {human}")
        } else if base.starts_with("OnUnitInactive") {
            format!("every {human} after idle")
        } else {
            format!("every {human}")
        });
    }
    None
}

/// 微秒时长的人类短语（parity：`humanDuration` 的取整规则）。
fn human_duration(usec: u64) -> String {
    let seconds = usec / 1_000_000;
    if seconds >= 86_400 {
        let days = seconds / 86_400;
        let hours = (seconds % 86_400) / 3_600;
        if hours > 0 {
            return format!("{days}d {hours}h");
        }
        return format!("{days}d");
    }
    if seconds >= 3_600 {
        let hours = seconds / 3_600;
        let minutes = (seconds % 3_600) / 60;
        if minutes > 0 {
            return format!("{hours}h {minutes}min");
        }
        return format!("{hours}h");
    }
    if seconds >= 60 {
        let minutes = seconds / 60;
        let rest = seconds % 60;
        if rest > 0 {
            return format!("{minutes}min {rest}s");
        }
        return format!("{minutes}min");
    }
    format!("{seconds}s")
}
