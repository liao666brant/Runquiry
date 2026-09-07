//! IP Helper 表的整表行解析与 socket / 端口条目构造（纯逻辑）。

use runquiry_core::{OpenPortEntry, Pid, SocketEntry, validate_socket_entry};

use super::wire::{
    SocketRow, TableKind, UDP_STATE, address_v4, address_v6, decode_port, tcp_state_name,
};

/// 行数上限（防御损坏表头驱动的无界解析；真实表远小于此）。
const MAX_ROWS: u32 = 65_536;

/// 表解析失败（稳定文案由调用方组装，不携带表内容）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpTableError {
    /// 表头不足 4 字节（dwNumEntries 读不出）。
    HeaderTooShort,
    /// 行数异常：表长与 `dwNumEntries` 推算长度不一致。
    RowCountMismatch {
        /// 表头声明的条目数。
        count: u32,
        /// 按条目数推算的所需字节数。
        needed: usize,
        /// 实际缓冲区字节数。
        actual: usize,
    },
    /// 条目数超出防御上限。
    RowCountExceedsCap {
        /// 表头声明的条目数。
        count: u32,
        /// 允许的最大条目数。
        cap: u32,
    },
}

/// 读取缓冲区内的 LE u32；越界返回 `None`。
fn read_u32_le(buf: &[u8], offset: usize) -> Option<u32> {
    if offset + 4 > buf.len() {
        return None;
    }
    Some(u32::from_le_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ]))
}

/// 解析一张 IP Helper 表为行列表（表头为 `dwNumEntries`，其后按行排布）。
///
/// # Errors
/// 表头过短 / 行数与表长不一致 / 条目数超上限时返回 [`IpTableError`]。
pub fn parse_table(kind: TableKind, buf: &[u8]) -> Result<Vec<SocketRow>, IpTableError> {
    let count = read_u32_le(buf, 0).ok_or(IpTableError::HeaderTooShort)?;
    if count > MAX_ROWS {
        return Err(IpTableError::RowCountExceedsCap {
            count,
            cap: MAX_ROWS,
        });
    }
    let row_bytes = kind.row_bytes();
    let needed = 4 + count as usize * row_bytes;
    if buf.len() < needed {
        return Err(IpTableError::RowCountMismatch {
            count,
            needed,
            actual: buf.len(),
        });
    }
    let mut rows = Vec::with_capacity(count as usize);
    for index in 0..count as usize {
        let base = 4 + index * row_bytes;
        let row = &buf[base..base + row_bytes];
        if let Some(parsed) = parse_row(kind, row) {
            rows.push(parsed);
        }
    }
    Ok(rows)
}

/// 解析单行；任一字段越界（损坏缓冲区）即整行丢弃。
fn parse_row(kind: TableKind, row: &[u8]) -> Option<SocketRow> {
    let (address, port_raw, remote, state_raw, owner_pid_raw) = match kind {
        TableKind::TcpV4 => (
            address_v4(row.get(4..8)?.try_into().ok()?),
            read_u32_le(row, 8)?,
            Some((address_v4(row.get(12..16)?.try_into().ok()?), read_u32_le(row, 16)?)),
            read_u32_le(row, 0)?,
            read_u32_le(row, 20)?,
        ),
        TableKind::TcpV6 => (
            address_v6(row.get(0..16)?.try_into().ok()?),
            read_u32_le(row, 20)?,
            Some((address_v6(row.get(24..40)?.try_into().ok()?), read_u32_le(row, 44)?)),
            read_u32_le(row, 48)?,
            read_u32_le(row, 52)?,
        ),
        TableKind::UdpV4 => (
            address_v4(row.get(0..4)?.try_into().ok()?),
            read_u32_le(row, 4)?,
            None,
            0,
            read_u32_le(row, 8)?,
        ),
        TableKind::UdpV6 => (
            address_v6(row.get(0..16)?.try_into().ok()?),
            read_u32_le(row, 20)?,
            None,
            0,
            read_u32_le(row, 24)?,
        ),
    };
    let state = if matches!(kind, TableKind::TcpV4 | TableKind::TcpV6) {
        tcp_state_name(state_raw)
    } else {
        UDP_STATE
    };
    Some(SocketRow {
        kind,
        address,
        port_raw,
        remote,
        state: String::from(state),
        owner_pid_raw,
    })
}

/// 行 → socket 条目：端口 0 / 越界拒绝（调用方跳过并记解析诊断），其余
/// 复用 core 的 `validate_socket_entry` 边界规则。
///
/// # Errors
/// 端口缺失或越界 / 边界规则违规时返回说明性错误。
pub fn to_socket_entry(
    row: &SocketRow,
    owner_pid: Option<Pid>,
) -> Result<Option<SocketEntry>, String> {
    let port = decode_port(row.port_raw).ok_or_else(|| {
        format!(
            "{:?} 行 {} 端口 DWORD {:#x} 非法（须为 1..=65535）",
            row.kind.protocol(),
            row.address,
            row.port_raw
        )
    })?;
    let entry = SocketEntry {
        inode: None,
        port: Some(port),
        address: row.address.clone(),
        remote_addr: row.remote.as_ref().and_then(|(addr, raw)| {
            decode_port(*raw).map(|port| format!("{addr}:{port}"))
        }),
        state: row.state.clone(),
        protocol: row.kind.protocol(),
        owner_pid,
    };
    validate_socket_entry(&entry)?;
    Ok(Some(entry))
}

/// 行 → 开放端口条目（`owner_pid` 由调用方判定：0 / 已退出 → `None`）。
///
/// # Errors
/// 端口非法时返回说明性错误。
pub fn to_open_port(row: &SocketRow, owner_pid: Option<Pid>) -> Result<OpenPortEntry, String> {
    let port = decode_port(row.port_raw).ok_or_else(|| {
        format!(
            "{:?} 行 {} 端口 DWORD {:#x} 非法",
            row.kind.protocol(),
            row.address,
            row.port_raw
        )
    })?;
    Ok(OpenPortEntry {
        pid: owner_pid,
        port,
        address: row.address.clone(),
        protocol: row.kind.protocol(),
        state: row.state.clone(),
    })
}