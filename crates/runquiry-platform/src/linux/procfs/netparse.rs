//! `/proc/net/{tcp,tcp6,udp,udp6,unix}` 与 fd inode 链接的文本解析。

use std::net::Ipv6Addr;

pub(in crate::linux) fn socket_state_name(hex: &str) -> String {
    match hex {
        "01" => String::from("ESTABLISHED"),
        "02" => String::from("SYN_SENT"),
        "03" => String::from("SYN_RECV"),
        "04" => String::from("FIN_WAIT1"),
        "05" => String::from("FIN_WAIT2"),
        "06" => String::from("TIME_WAIT"),
        "07" => String::from("CLOSE"),
        "08" => String::from("CLOSE_WAIT"),
        "09" => String::from("LAST_ACK"),
        "0A" => String::from("LISTEN"),
        "0B" => String::from("CLOSING"),
        _ => String::from("UNKNOWN"),
    }
}

/// Unix domain socket 内核状态 → 状态名（`/proc/net/unix` 的 St 列）。
pub(in crate::linux) fn unix_socket_state(st: &str) -> String {
    match st {
        "01" => String::from("UNCONNECTED"),
        "02" => String::from("CONNECTING"),
        "03" => String::from("CONNECTED"),
        "04" => String::from("DISCONNECTING"),
        _ => String::from("UNKNOWN"),
    }
}

/// `/proc/net/{tcp,tcp6,udp,udp6}` 单行解析结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::linux) struct InetSocketRow {
    /// 关联 inode。
    pub(in crate::linux) inode: u64,
    /// 点分地址（IPv4 点分十进制 / IPv6 压缩形式）。
    pub(in crate::linux) address: String,
    /// 远端地址（无连接或不可得为 `None`）。
    pub(in crate::linux) remote_addr: Option<String>,
    /// 十六进制端口原值（0 表示无效端口，由调用方记诊断并跳过）。
    pub(in crate::linux) port_raw: u16,
    /// 状态名（未知码记 UNKNOWN）。
    pub(in crate::linux) state: String,
}

/// 解析一张 `/proc/net` inet 表（跳过表头；列数不足或地址不合法的行跳过）。
///
/// 地址列形如 `0100007F:1F90`：IP 为十六进制小端序，端口为十六进制大端序
/// （witr `parseAddr`）。
pub(in crate::linux) fn parse_inet_table(raw: &str, ipv6: bool) -> Vec<InetSocketRow> {
    let mut rows = Vec::new();
    for line in raw.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 10 {
            continue;
        }
        let Some((ip_hex, port_hex)) = fields[1].rsplit_once(':') else {
            continue;
        };
        let Some(address) = parse_ip_hex(ip_hex, ipv6) else {
            continue;
        };
        // 端口非法（非十六进制）按 0 处理：调用方对 port 0 记诊断并跳过该行。
        let port_raw = u16::from_str_radix(port_hex, 16).unwrap_or(0);
        let remote_addr = fields[2].rsplit_once(':').and_then(|(rip, rport)| {
            // 远端端口为 0（无连接）时按契约置 None。
            let rport = u16::from_str_radix(rport, 16).ok()?;
            if rport == 0 {
                return None;
            }
            let addr = parse_ip_hex(rip, ipv6)?;
            Some(format!("{addr}:{rport}"))
        });
        rows.push(InetSocketRow {
            inode: fields[9].parse::<u64>().unwrap_or(0),
            port_raw,
            address,
            state: socket_state_name(fields[3]),
            remote_addr,
        });
    }
    rows
}

/// 解析 `/proc/net` 的十六进制地址：IPv4 为 8 位十六进制（首字节是地址的
/// 末 octet）；IPv6 为 32 位十六进制、每 4 字节组小端序（witr `parseAddr`
/// 的 Rust 等价实现）。
fn parse_ip_hex(ip_hex: &str, ipv6: bool) -> Option<String> {
    let bytes = decode_hex(ip_hex)?;
    let octet = |idx: usize| bytes.get(idx).copied().unwrap_or(0);
    if ipv6 {
        if bytes.len() != 16 {
            return Some(String::from("::"));
        }
        let mut octets = [0u8; 16];
        for group in 0..4 {
            for (offset, byte) in bytes[group * 4..group * 4 + 4].iter().rev().enumerate() {
                octets[group * 4 + offset] = *byte;
            }
        }
        Some(Ipv6Addr::from(octets).to_string())
    } else {
        // 内核把 u32 地址按小端序写为字节流：首字节是地址的最后一 octet。
        Some(format!(
            "{}.{}.{}.{}",
            octet(3),
            octet(2),
            octet(1),
            octet(0)
        ))
    }
}

/// 十六进制串 → 字节（奇数长度或非法字符返回 `None`）。
fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|idx| u8::from_str_radix(&hex[idx..idx + 2], 16).ok())
        .collect()
}

/// `/proc/net/unix` 单行解析结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::linux) struct UnixSocketRow {
    /// 关联 inode。
    pub(in crate::linux) inode: u64,
    /// 绑定路径（匿名 socket 为空串）。
    pub(in crate::linux) path: String,
    /// 状态名。
    pub(in crate::linux) state: String,
}

/// 解析 `/proc/net/unix`（列：`Num` / `RefCount` / `Protocol` / `Flags` / `Type` / `St` / `Inode` / `Path`；
/// 路径可能含空格且可缺失）。非十进制 inode 的行跳过。
pub(in crate::linux) fn parse_unix_table(raw: &str) -> Vec<UnixSocketRow> {
    let mut rows = Vec::new();
    for line in raw.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 7 {
            continue;
        }
        let Some(inode) = fields[6].parse::<u64>().ok().filter(|v| *v > 0) else {
            continue;
        };
        rows.push(UnixSocketRow {
            inode,
            path: fields[7..].join(" "),
            state: unix_socket_state(fields[5]),
        });
    }
    rows
}

/// `socket:[inode]` 形式的 fd 链接目标 → inode。
pub(in crate::linux) fn parse_socket_inode(link: &str) -> Option<u64> {
    link.strip_prefix("socket:[")
        .and_then(|rest| rest.strip_suffix(']'))
        .and_then(|inode| inode.parse::<u64>().ok())
}
