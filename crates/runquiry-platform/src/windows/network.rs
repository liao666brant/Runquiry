//! Windows 网络采集：`NetworkInventory` 的 IP Helper 实现。
//!
//! `GetExtendedTcpTable(TCP_TABLE_OWNER_PID_ALL)`（含监听行，v4+v6）与
//! `GetExtendedUdpTable(UDP_TABLE_OWNER_PID)`（v4+v6），两阶段 sizing
//! （先探询尺寸再读取，缓冲不足按新尺寸重试一次）。行解码与条目构造
//! 全部复用纯解析模块 `ip_table`；属主 PID 0 / 已退出 → `None` + 聚合
//! 诊断，条目不丢弃、不伪造属主。

use std::collections::HashSet;

use runquiry_core::{
    CapabilityStatus, DiagnosticCode, DiagnosticIssue, Inspection, NetworkInventory, OpenPortEntry,
    Pid, Port, SocketEntry,
};

use super::WindowsPlatform;
use super::ip_table::{self, TableKind};
use super::winerror::{self, Win32Error};

/// TCP 表类（ALL 含监听行；LISTEN 变体是 ALL 的子集，单独查询只会产生
/// 重复条目，故不再调用——见交付报告披露）。
const TCP_TABLE_OWNER_PID_ALL: i32 = 5;
/// UDP 表类。
const UDP_TABLE_OWNER_PID: i32 = 1;
/// IPv4 / IPv6 地址族（WinSock AF_INET / AF_INET6）。
const AF_INET: u32 = 2;
const AF_INET6: u32 = 23;

/// 表抓取失败（尺寸探测 / 读取 / 持续变化分别给出稳定文案）。
#[derive(Debug, Clone, Copy)]
enum TableFetchError {
    /// 尺寸探测失败（含 Win32 错误码）。
    Probe(Win32Error),
    /// 读取失败（含 Win32 错误码）。
    Read(Win32Error),
    /// 两次读取间表持续变化，重试后仍失败。
    Volatile,
}

impl TableFetchError {
    fn diagnostic(self, kind: TableKind) -> DiagnosticIssue {
        let code = match self {
            Self::Probe(error) | Self::Read(error) if error.is_access_denied() => {
                DiagnosticCode::PermissionDenied
            }
            Self::Volatile => DiagnosticCode::Timeout,
            Self::Probe(_) | Self::Read(_) => DiagnosticCode::Unknown,
        };
        let reason = match self {
            Self::Probe(error) | Self::Read(error) => {
                format!("GetExtended*Table 失败（Win32 错误码 {}）", error.0)
            }
            Self::Volatile => String::from("表在两次读取间持续变化"),
        };
        DiagnosticIssue::new(code, format!("{kind:?} 表：{reason}"))
    }
}

/// 一次扩展表抓取：NULL 探询尺寸 → 分配 → 读取，不足时按新尺寸重试一次。
fn fetch_table(
    table_class: i32,
    address_family: u32,
    udp: bool,
) -> Result<Vec<u8>, TableFetchError> {
    let mut size: u32 = 0;
    let probe = if udp {
        // SAFETY:第一阶段以空缓冲区探询尺寸（API 契约：pdwsize 出参写入所需
        // 字节数，返回 ERROR_INSUFFICIENT_BUFFER / ERROR_MORE_DATA）。
        unsafe {
            windows_sys::Win32::NetworkManagement::IpHelper::GetExtendedUdpTable(
                core::ptr::null_mut(),
                &mut size,
                0,
                address_family,
                table_class,
                0,
            )
        }
    } else {
        // SAFETY:同上，仅换用 TCP 表 API，参数契约一致。
        unsafe {
            windows_sys::Win32::NetworkManagement::IpHelper::GetExtendedTcpTable(
                core::ptr::null_mut(),
                &mut size,
                0,
                address_family,
                table_class,
                0,
            )
        }
    };
    if probe == winerror::ERROR_SUCCESS && size == 0 {
        // 空表：API 未报告尺寸，按 4 字节空表头处理。
        return Ok(vec![0, 0, 0, 0]);
    }
    if size == 0 {
        return Err(TableFetchError::Probe(Win32Error(probe)));
    }
    let mut attempts = 0;
    while attempts < 2 {
        attempts += 1;
        let bytes = usize::try_from(size).unwrap_or(0);
        let mut buffer = vec![0u8; bytes];
        // pdwSize 出参必须用可存活的变量承接：ERROR_INSUFFICIENT_BUFFER 时
        // API 会写入实际所需字节数，重试必须按新尺寸分配（临时值会丢失出参）。
        let mut needed = size;
        let written = if udp {
            // SAFETY:buffer 为本函数分配的可写内存，长度 size 与 API 的
            // pdwsize 约定一致；MIB 表内字段按 4 字节对齐写入，缓冲区起点由
            // Rust 分配器保证 ≥ 8 字节对齐（Vec<u8> 实际布局依赖分配器，此处
            // 读取一律经由缓冲区字节拷贝，不做结构体重解释）。
            unsafe {
                windows_sys::Win32::NetworkManagement::IpHelper::GetExtendedUdpTable(
                    buffer.as_mut_ptr().cast::<core::ffi::c_void>(),
                    &mut needed,
                    0,
                    address_family,
                    table_class,
                    0,
                )
            }
        } else {
            // SAFETY:同上，仅换用 TCP 表 API，参数契约一致。
            unsafe {
                windows_sys::Win32::NetworkManagement::IpHelper::GetExtendedTcpTable(
                    buffer.as_mut_ptr().cast::<core::ffi::c_void>(),
                    &mut needed,
                    0,
                    address_family,
                    table_class,
                    0,
                )
            }
        };
        if written == winerror::ERROR_SUCCESS {
            return Ok(buffer);
        }
        if written == winerror::ERROR_INSUFFICIENT_BUFFER || written == winerror::ERROR_MORE_DATA {
            // 表在两次读取间增长：按系统报告的新尺寸重试一次。
            size = needed;
            continue;
        }
        return Err(TableFetchError::Read(Win32Error(written)));
    }
    Err(TableFetchError::Volatile)
}

impl WindowsPlatform {
    /// 解析四类表为 (行, 诊断)；整体不可得时 `None`（调用方返回完全失败）。
    fn collect_rows(&self) -> Option<(Vec<ip_table::SocketRow>, Vec<DiagnosticIssue>)> {
        let requests = [
            (TableKind::TcpV4, TCP_TABLE_OWNER_PID_ALL, AF_INET, false),
            (TableKind::TcpV6, TCP_TABLE_OWNER_PID_ALL, AF_INET6, false),
            (TableKind::UdpV4, UDP_TABLE_OWNER_PID, AF_INET, true),
            (TableKind::UdpV6, UDP_TABLE_OWNER_PID, AF_INET6, true),
        ];
        let mut rows = Vec::new();
        let mut issues = Vec::new();
        for (kind, class, family, udp) in requests {
            match fetch_table(class, family, udp) {
                Ok(buffer) => match ip_table::parse_table(kind, &buffer) {
                    Ok(parsed) => rows.extend(parsed),
                    Err(error) => issues.push(parse_error_diagnostic(kind, error)),
                },
                Err(error) => issues.push(error.diagnostic(kind)),
            }
        }
        if rows.is_empty() && issues.len() == requests.len() {
            return None;
        }
        Some((rows, issues))
    }

    /// 行 → 条目：属主判定（PID 0 → None + 聚合诊断；端口 0 行跳过并记
    /// 解析诊断），`(pid, port, address, protocol, state)` 去重。
    fn entries_of(
        rows: Vec<ip_table::SocketRow>,
        issues: &mut Vec<DiagnosticIssue>,
        filter_pid: Option<Pid>,
    ) -> Vec<(SocketEntry, Port)> {
        let mut entries = Vec::new();
        let mut seen = HashSet::new();
        let mut unowned = 0usize;
        let mut malformed = 0usize;
        for row in &rows {
            let Some(port) = ip_table::decode_port(row.port_raw) else {
                malformed += 1;
                continue;
            };
            let owner = if row.owner_pid_raw == 0 {
                None
            } else {
                Pid::new(row.owner_pid_raw).ok()
            };
            if owner.is_none() {
                unowned += 1;
            }
            if let Some(filter) = filter_pid
                && owner != Some(filter)
            {
                continue;
            }
            let entry = match ip_table::to_socket_entry(row, owner) {
                Ok(Some(entry)) => entry,
                Ok(None) | Err(_) => {
                    malformed += 1;
                    continue;
                }
            };
            if seen.insert((entry.owner_pid, port, entry.address.clone(), entry.protocol)) {
                entries.push((entry, port));
            }
        }
        if malformed > 0 {
            issues.push(DiagnosticIssue::new(
                DiagnosticCode::ParseFailed,
                format!("{malformed} 条 socket 行端口非法或校验失败，已跳过"),
            ));
        }
        if unowned > 0 {
            issues.push(DiagnosticIssue::new(
                DiagnosticCode::PermissionDenied,
                format!("{unowned} 条 socket 条目属主不可知（PID 0 或已退出）"),
            ));
        }
        entries
    }
}

/// 表解析错误 → 诊断（文案稳定，不含表内容）。
fn parse_error_diagnostic(kind: TableKind, error: ip_table::IpTableError) -> DiagnosticIssue {
    let reason = match error {
        ip_table::IpTableError::HeaderTooShort => String::from("表头不足（dwNumEntries 读不出）"),
        ip_table::IpTableError::RowCountMismatch {
            count,
            needed,
            actual,
        } => {
            format!("行数与表长不一致：{count} 条 × 行长需要 {needed} 字节，实际 {actual}")
        }
        ip_table::IpTableError::RowCountExceedsCap { count, cap } => {
            format!("条目数 {count} 超出防御上限 {cap}")
        }
    };
    DiagnosticIssue::new(
        DiagnosticCode::ParseFailed,
        format!("{kind:?} 表：{reason}"),
    )
}

impl NetworkInventory for WindowsPlatform {
    fn capability(&self) -> CapabilityStatus {
        CapabilityStatus::Supported
    }

    fn open_ports(&self) -> Inspection<Vec<OpenPortEntry>> {
        let Some((rows, mut issues)) = self.collect_rows() else {
            return Inspection::failed(vec![DiagnosticIssue::new(
                DiagnosticCode::Unknown,
                String::from("四张 IP Helper 表均不可读，端口清单不可得"),
            )]);
        };
        let entries = Self::entries_of(rows, &mut issues, None);
        let ports: Vec<OpenPortEntry> = entries
            .into_iter()
            .map(|(entry, port)| OpenPortEntry {
                pid: entry.owner_pid,
                port,
                address: entry.address.clone(),
                protocol: entry.protocol,
                state: entry.state.clone(),
            })
            .collect();
        if issues.is_empty() {
            Inspection::complete(ports)
        } else {
            Inspection::partial(ports, issues)
        }
    }

    fn sockets_of(&self, pid: Pid) -> Inspection<Vec<SocketEntry>> {
        let Some((rows, mut issues)) = self.collect_rows() else {
            return Inspection::failed(vec![DiagnosticIssue::new(
                DiagnosticCode::Unknown,
                format!("进程 {pid} 的 IP Helper 表均不可读"),
            )]);
        };
        let entries = Self::entries_of(rows, &mut issues, Some(pid));
        let sockets: Vec<SocketEntry> = entries.into_iter().map(|(entry, _)| entry).collect();
        if issues.is_empty() {
            Inspection::complete(sockets)
        } else {
            Inspection::partial(sockets, issues)
        }
    }
}
