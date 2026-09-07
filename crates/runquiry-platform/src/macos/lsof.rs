//! macOS `lsof` 输出解析（纯逻辑：只依赖 std 与 runquiry-core 类型）。
//!
//! 本模块不得引用 sysinfo / libc / 命令执行：集成测试以 `#[path]` 在 Linux
//! 直接编译本模块（C1 无法在本机编译 macOS cfg 代码的可测性边界）。
//!
//! 语义逐字对齐 witr（行为契约静态阅读来源）：
//! * `net_darwin.go::ListOpenPorts / parseNetstatAddr`（`lsof -i -P -n` 列格式）；
//! * `openfiles_darwin.go / locks_darwin.go`（`lsof -l -n -P` 列格式）；
//! * `process_darwin.go::getCwdAndBinaryPath`（`lsof -F fn`）；
//! * `target/file_darwin.go::ResolveFile`（`lsof -F p <path>`）。
//!
//! 已知边界（witr 语义如实保留，不做平台外修补）：lsof 未使用 `-0` / 转义
//! 策略，**含换行的路径会被按行切分而不可靠**；含空格的路径经「字段重拼接」
//! 恢复（witr `strings.Join(fields[8:], " ")` 同语义）；`-F` 行内的换行同样
//! 截断。属主不可知行按 core 端口后置条件返回 `pid: None` 条目并记诊断
//! （witr 静默跳过，为满足 `NetworkInventory` 契约而如实保留条目）。

use std::path::PathBuf;

use runquiry_core::{LockMode, Protocol};

/// `lsof -i -P -n` 的单行解析结果（端口/地址/协议已定形，PID 可缺）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawOpenPort {
    /// 归因到的进程；行内 PID 不可解析时为 `None`（属主不可知，不丢弃）。
    pub pid: Option<u32>,
    /// 端口号（端口 0 的行不入结果，记解析诊断）。
    pub port: u16,
    /// 地址（`*` 统一为 `0.0.0.0`，witr 同语义）。
    pub address: String,
    /// 协议（IPv6 字面量映射 `Tcp6` / `Udp6`）。
    pub protocol: Protocol,
    /// Socket 状态（UDP 无状态列按 witr 记 `OPEN`）。
    pub state: String,
}

/// `lsof -l -n -P` 的单行解析结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawFileRow {
    /// 持有者 PID。
    pub pid: u32,
    /// 持有者进程名（COMMAND 列）。
    pub process: String,
    /// 数字 FD（`cwd` / `txt` / `mem` 等非数字 FD 为 `None`）。
    pub fd: Option<u32>,
    /// TYPE 列（`REG` / `DIR` / `IPv4` 等，原样保留）。
    pub fd_type: String,
    /// NAME 列字段重拼接后的路径（含空格路径可恢复）。
    pub path: String,
    /// FD 列末字符的锁标志映射（无锁标志为 `None`）。
    pub lock_mode: Option<LockMode>,
}

/// 解析 `lsof -i -P -n` 列格式输出为开放端口行。
///
/// 返回 `(成功行, 失败原因)`：失败原因供调用方记
/// [`runquiry_core::DiagnosticIssue`]；地址/端口/协议非法的行无法定形为
/// `OpenPortEntry`，只能跳过并记诊断（不得伪造端口）。
#[must_use]
pub(crate) fn parse_open_ports(stdout: &str) -> (Vec<RawOpenPort>, Vec<String>) {
    let mut rows = Vec::new();
    let mut issues = Vec::new();
    let lines: Vec<&str> = stdout.lines().collect();
    // 首行可能是表头（witr：COMMAND 前缀跳过）。
    let body = match lines.first() {
        Some(first) if first.starts_with("COMMAND") => &lines[1..],
        _ => &lines[..],
    };
    for line in body {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 9 {
            issues.push(format!("lsof -i 行字段少于 9 列，跳过：{line}"));
            continue;
        }
        // NODE 列（fields[7]）应为 TCP/UDP；非 TCP/UDP 的行模型无法承载
        //（witr 记 UNKNOWN，此处按已披露偏差跳过并记诊断）。
        let node = fields[7];
        let name = fields[8];
        let pid = match fields[1].parse::<u32>() {
            Ok(pid) if pid > 0 => Some(pid),
            _ => {
                issues.push(format!("lsof -i 行 PID 不可解析，属主记为 None：{line}"));
                None
            }
        };
        let state = fields.get(9).map_or_else(
            || {
                if node.eq_ignore_ascii_case("UDP") {
                    String::from("OPEN")
                } else {
                    String::from("UNKNOWN")
                }
            },
            |raw| raw.trim_matches(|c| c == '(' || c == ')').to_string(),
        );
        let Some((address, port)) = parse_netstat_addr(name) else {
            issues.push(format!("lsof -i 行地址无法解析，跳过：{line}"));
            continue;
        };
        // TYPE 列（fields[4]）是套接字族的权威来源：witr 的 Protocol 为
        // TCP/UDP 字符串，Runquiry 模型区分 v4/v6，通配地址文本无法判定族。
        let is_v6 = fields
            .get(4)
            .is_some_and(|kind| kind.eq_ignore_ascii_case("IPv6"));
        let Some(protocol) = protocol_of(node, is_v6) else {
            issues.push(format!("lsof -i 行协议既非 TCP 也非 UDP，跳过：{line}"));
            continue;
        };
        if port == 0 {
            issues.push(format!("lsof -i 行端口为 0（非法），跳过：{line}"));
            continue;
        }
        rows.push(RawOpenPort {
            pid,
            port,
            address,
            protocol,
            state,
        });
    }
    (rows, issues)
}

/// 解析 lsof 地址列（witr `parseNetstatAddr`）：支持 `*:8080` / `*.8080`、
/// `[::1]:8080`、`127.0.0.1:8080` 与 macOS netstat 点分 `127.0.0.1.8080`。
#[must_use]
pub(crate) fn parse_netstat_addr(name: &str) -> Option<(String, u16)> {
    // IPv6 括号形式：`[::1]:8080` 或 `[fe80::1%en0].8080`。
    if let Some(rest) = name.strip_prefix('[') {
        let bracket_end = rest.rfind(']')?;
        let ip = rest[..bracket_end].to_string();
        let tail = &rest[bracket_end + 1..];
        let port_str = tail.strip_prefix(':').or_else(|| tail.strip_prefix('.'))?;
        let port = port_str.parse::<u16>().ok()?;
        return Some((ip, port));
    }
    // 通配形式：`*:8080` 或 `*.8080`（witr：`*` 记为 `0.0.0.0`）。
    if let Some(rest) = name.strip_prefix('*') {
        let port_str = rest.strip_prefix(':').or_else(|| rest.strip_prefix('.'))?;
        let port = port_str.parse::<u16>().ok()?;
        return Some((String::from("0.0.0.0"), port));
    }
    // 标准冒号形式：`127.0.0.1:8080`。
    if let Some(idx) = name.rfind(':')
        && let Ok(port) = name[idx + 1..].parse::<u16>()
    {
        return Some((name[..idx].to_string(), port));
    }
    // macOS netstat 点分形式：`127.0.0.1.8080`。
    if let Some(idx) = name.rfind('.')
        && let Ok(port) = name[idx + 1..].parse::<u16>()
    {
        return Some((name[..idx].to_string(), port));
    }
    None
}

/// NODE 列 + 地址 → core 协议：地址含 `:` 或以 `[` 开头按 IPv6 归类。
///
/// witr 原样保留 `TCP`/`UDP` 字符串；core 模型区分 v4/v6，这里按地址形状
/// 归类（属平台适配，见交付报告披露）。
fn protocol_of(node: &str, is_v6: bool) -> Option<Protocol> {
    match node {
        "TCP" if !is_v6 => Some(Protocol::Tcp),
        "TCP" => Some(Protocol::Tcp6),
        "UDP" if !is_v6 => Some(Protocol::Udp),
        "UDP" => Some(Protocol::Udp6),
        _ => None,
    }
}

/// 解析 `lsof -l -n -P` 列格式输出为文件行（witr `ListAllOpenFiles` /
/// `ListLockedFiles` 共用行解析）。
///
/// 返回 `(成功行, 失败原因)`；PID 不可解析的行跳过并记诊断。
#[must_use]
pub(crate) fn parse_file_rows(stdout: &str) -> (Vec<RawFileRow>, Vec<String>) {
    let mut rows = Vec::new();
    let mut issues = Vec::new();
    let mut header_skipped = false;
    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        // COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME（≥9 列）。
        if fields.len() < 9 {
            issues.push(format!("lsof 行字段少于 9 列，跳过：{line}"));
            continue;
        }
        // 首行是表头：COMMAND PID USER FD TYPE ...（第 2 列非数字）。
        if !header_skipped && fields[1].parse::<u32>().is_err() {
            header_skipped = true;
            continue;
        }
        let Some(pid) = fields[1].parse::<u32>().ok().filter(|pid| *pid > 0) else {
            issues.push(format!("lsof 行 PID 不可解析，跳过：{line}"));
            continue;
        };
        rows.push(RawFileRow {
            pid,
            process: fields[0].to_string(),
            fd: fd_number(fields[3]),
            fd_type: fields[4].to_string(),
            path: fields[8..].join(" "),
            lock_mode: fd_lock_mode(fields[3]),
        });
    }
    (rows, issues)
}

/// 数字 FD 列 → FD 号（`3uW` → `3`；`cwd` / `txt` 等非数字为 `None`）。
#[must_use]
pub(crate) fn fd_number(fd_column: &str) -> Option<u32> {
    let digits: String = fd_column
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok().filter(|fd| *fd > 0)
}

/// FD 列末字符的锁标志映射（witr `lsofFDLockMode`）。
///
/// 注意：lsof 的访问模式与锁标志共用小写字母（`3u` 既可能是读写访问也可能
/// 是区域读写锁），witr 按末字符如实映射——该歧义在 macOS 上不可由 lsof 输
/// 出消除，属已披露的 best-effort 边界。
#[must_use]
pub(crate) fn fd_lock_mode(fd_column: &str) -> Option<LockMode> {
    match fd_column.chars().last()? {
        'W' | 'w' => Some(LockMode::Write),
        'R' | 'r' => Some(LockMode::Read),
        'u' | 'U' => Some(LockMode::ReadWrite),
        _ => None,
    }
}

/// 解析 `lsof -a -p <pid> -d cwd,txt -F fn` 输出（witr `getCwdAndBinaryPath`）：
/// 返回 `(cwd, txt 路径)`。
#[must_use]
pub(crate) fn parse_cwd_txt(stdout: &str) -> (Option<PathBuf>, Option<PathBuf>) {
    let mut cwd = None;
    let mut bin = None;
    let mut current_fd = String::new();
    for line in stdout.lines() {
        if line.chars().count() < 2 {
            continue;
        }
        let mut chars = line.chars();
        let tag = chars.next().unwrap_or_default();
        let value = chars.as_str().trim();
        match tag {
            'f' => current_fd = value.trim().to_string(),
            'n' => match current_fd.as_str() {
                "cwd" => cwd = Some(PathBuf::from(value.trim())),
                "txt" if bin.is_none() => bin = Some(PathBuf::from(value.trim())),
                _ => {}
            },
            _ => {}
        }
    }
    (cwd, bin)
}

/// 解析 `lsof -F p <path>` 输出为持有者 PID 列表（witr `ResolveFile`）。
#[must_use]
pub(crate) fn parse_holder_pids(stdout: &str) -> Vec<u32> {
    stdout
        .lines()
        .filter_map(|line| line.trim().strip_prefix('p'))
        .filter_map(|raw| raw.parse::<u32>().ok())
        .filter(|pid| *pid > 0)
        .collect()
}

/// 「值得呈现的路径」过滤（witr `isInterestingDarwinPath`）：丢弃空路径与
/// `/dev/null`、`/dev/tty*`。
#[must_use]
pub(crate) fn is_interesting_path(path: &str) -> bool {
    !(path.is_empty()
        || path == "/dev/null"
        || path.starts_with("/dev/tty")
        || path.starts_with("/dev/ttys"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn netstat_addr_covers_all_lsof_shapes() {
        assert_eq!(
            parse_netstat_addr("*.8080"),
            Some((String::from("0.0.0.0"), 8080))
        );
        assert_eq!(
            parse_netstat_addr("[::1]:8080"),
            Some((String::from("::1"), 8080))
        );
        assert_eq!(
            parse_netstat_addr("127.0.0.1:8080"),
            Some((String::from("127.0.0.1"), 8080))
        );
        assert_eq!(
            parse_netstat_addr("127.0.0.1.8080"),
            Some((String::from("127.0.0.1"), 8080))
        );
        assert_eq!(parse_netstat_addr("*"), None);
    }
}
