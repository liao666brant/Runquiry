//! `/proc/PID/*` 文本解析：stat、status、environ、fd limit、I/O、内存与启动时间。

use std::time::{Duration, SystemTime};

use runquiry_core::{IoStats, MemoryInfo};

/// Linux 时钟频率（`CLK_TCK`）：witr `ticksPerSecond()` 同值约定（恒 100）。
pub(in crate::linux) const CLK_TCK: u64 = 100;

/// 页大小（字节）：witr 以 `os.Getpagesize()` 取内核值；本 crate 无 libc 依赖，
/// 固定按主流 4K 内核换算（大页内核上 RSS 类字段按 4K 缩放，见报告已知限制）。
pub(in crate::linux) const PAGE_SIZE: u64 = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::linux) struct StatInfo {
    /// 命令名（comm，可能含空格与括号）。
    pub(in crate::linux) comm: String,
    /// 单字符进程状态（R/S/D/Z/T/X…）。
    pub(in crate::linux) state: char,
    /// 父进程 ID。
    pub(in crate::linux) ppid: u32,
    /// 用户态 CPU ticks。
    pub(in crate::linux) utime: u64,
    /// 内核态 CPU ticks。
    pub(in crate::linux) stime: u64,
    /// 开机起的启动时钟 tick 数（stat 第 22 字段）。
    pub(in crate::linux) start_ticks: u64,
    /// RSS 页数。
    pub(in crate::linux) rss_pages: u64,
}

/// 解析 `/proc/PID/stat`：comm 可能含空格与括号，须以首个 `(` 与最后一个 `)`
/// 定界（witr `ReadProcess` 同款边界检查：字段不足 22 视为格式损坏）。
pub(in crate::linux) fn parse_stat(raw: &str) -> Option<StatInfo> {
    let open = raw.find('(')?;
    let close = raw.rfind(')')?;
    if close <= open || raw[close + 1..].is_empty() {
        return None;
    }
    let comm = String::from(&raw[open + 1..close]);
    let rest = &raw[close + 1..];
    let fields: Vec<&str> = rest.split_whitespace().collect();
    // after-comm 字段：0=state 1=ppid 11=utime 12=stime 19=starttime 21=rss。
    if fields.len() < 22 {
        return None;
    }
    let state = fields[0].chars().next()?;
    let ppid = fields[1].parse::<u32>().ok()?;
    let utime = fields[11].parse::<u64>().ok()?;
    let stime = fields[12].parse::<u64>().ok()?;
    let start_ticks = fields[19].parse::<u64>().ok()?;
    let rss_pages = fields[21].parse::<u64>().ok()?;
    Some(StatInfo {
        comm,
        state,
        ppid,
        utime,
        stime,
        start_ticks,
        rss_pages,
    })
}

/// `/proc/PID/status` 解析结果（`Name` / `Uid` / `CapEff`）。
#[derive(Debug, Clone, Default)]
pub(in crate::linux) struct StatusInfo {
    /// status 的 Name（与 comm 可能截断差异，仅作回退）。
    pub(in crate::linux) name: Option<String>,
    /// 真实 UID（Uid 行第一列）。
    pub(in crate::linux) uid: Option<u32>,
    /// 有效 capabilities 十六进制原值。
    pub(in crate::linux) cap_eff_hex: Option<String>,
}

/// 解析 `/proc/PID/status` 的 `Name` / `Uid` / `CapEff` 行（其余行忽略）。
pub(in crate::linux) fn parse_status(raw: &str) -> StatusInfo {
    let mut info = StatusInfo::default();
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("Name:") {
            info.name = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("Uid:") {
            info.uid = rest
                .split_whitespace()
                .next()
                .and_then(|uid| uid.parse::<u32>().ok());
        } else if let Some(rest) = line.strip_prefix("CapEff:") {
            info.cap_eff_hex = Some(rest.trim().to_string());
        }
    }
    info
}

/// 解析 `\0` 分隔的列表（cmdline / environ 原文），丢弃空段。
pub(in crate::linux) fn parse_null_list(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect()
}

/// 解析 environ 原文为键值对（无 `=` 的段跳过）。
pub(in crate::linux) fn parse_environ(bytes: &[u8]) -> Vec<(String, String)> {
    parse_environ_checked(bytes).0
}

/// 解析 environ 并标记无效 UTF-8 或缺少 `=` 的非空条目。
pub(in crate::linux) fn parse_environ_checked(bytes: &[u8]) -> (Vec<(String, String)>, bool) {
    let mut environment = Vec::new();
    let mut valid = true;
    for entry in bytes
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let Ok(entry) = std::str::from_utf8(entry) else {
            valid = false;
            continue;
        };
        let Some((key, value)) = entry.split_once('=') else {
            valid = false;
            continue;
        };
        environment.push((String::from(key), String::from(value)));
    }
    (environment, valid)
}

/// 解析 `/proc/PID/limits` 的 "Max open files" 软限制（witr `getFileLimit`）：
/// `unlimited` 记 0；行缺失或不可解析返回 `None`。
pub(in crate::linux) fn parse_fd_limit(raw: &str) -> Option<u64> {
    for line in raw.lines() {
        if !line.starts_with("Max open files") {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        let soft = fields.get(3)?;
        return if *soft == "unlimited" {
            Some(0)
        } else {
            soft.parse::<u64>().ok()
        };
    }
    None
}

/// 解析 `/proc/PID/io`（`read_bytes` / `write_bytes` / `syscr` / `syscw`）。
pub(in crate::linux) fn parse_io(raw: &str) -> Option<IoStats> {
    let mut stats = IoStats::default();
    let mut parsed = false;
    for line in raw.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let Ok(value) = value.trim().parse::<u64>() else {
            continue;
        };
        match key.trim() {
            "read_bytes" => {
                stats.read_bytes = value;
                parsed = true;
            }
            "write_bytes" => {
                stats.write_bytes = value;
                parsed = true;
            }
            "syscr" => {
                stats.read_ops = value;
                parsed = true;
            }
            "syscw" => {
                stats.write_ops = value;
                parsed = true;
            }
            _ => {}
        }
    }
    parsed.then_some(stats)
}

/// 解析 `/proc/PID/statm` 为详细内存信息（7 字段 × 页大小，witr `ReadExtendedInfo`）。
pub(in crate::linux) fn parse_statm(raw: &str) -> Option<MemoryInfo> {
    let fields: Vec<u64> = raw
        .split_whitespace()
        .map(str::parse::<u64>)
        .collect::<Result<_, _>>()
        .ok()?;
    if fields.len() < 7 {
        return None;
    }
    let scale = |pages: u64| pages.saturating_mul(PAGE_SIZE);
    Some(MemoryInfo {
        vms_bytes: scale(fields[0]),
        rss_bytes: scale(fields[1]),
        shared_bytes: scale(fields[2]),
        text_bytes: scale(fields[3]),
        lib_bytes: scale(fields[4]),
        data_bytes: scale(fields[5]),
        dirty_bytes: scale(fields[6]),
    })
}

/// 解析 `/proc/meminfo` 的 `MemTotal`（kB → 字节）；不可得返回 `None`。
pub(in crate::linux) fn parse_meminfo_total(raw: &str) -> Option<u64> {
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            let kb = rest.split_whitespace().next()?.parse::<u64>().ok()?;
            return Some(kb.saturating_mul(1024));
        }
    }
    None
}

/// 解析 `/proc/stat` 的 `btime`（秒，witr `bootTime`）；不可得返回 `None`。
pub(in crate::linux) fn parse_boot_time(proc_stat: &str) -> Option<SystemTime> {
    for line in proc_stat.lines() {
        if let Some(rest) = line.strip_prefix("btime") {
            let secs = rest.split_whitespace().next()?.parse::<u64>().ok()?;
            return SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(secs));
        }
    }
    None
}

/// 把 stat 的启动 tick 换算为绝对时间（witr `startTimeFromTicks`：先除后乘
/// 防 int64 溢出，除不尽部分按 hz 折算纳秒）。boot 晚于 epoch 才可换算。
pub(in crate::linux) fn start_time_from_ticks(
    boot: SystemTime,
    start_ticks: u64,
) -> Option<SystemTime> {
    let secs = start_ticks / CLK_TCK;
    let nsec = (start_ticks % CLK_TCK) * 1_000_000_000 / CLK_TCK;
    boot.checked_add(Duration::new(secs, u32::try_from(nsec).unwrap_or(u32::MAX)))
}
