//! Linux 网络与 Socket 采集：`NetworkInventory` 的 `/proc/net` 真实只读实现。
//!
//! 四张 inet 表（tcp/tcp6/udp/udp6）+ Unix 表全部经可注入根目录解析；
//! FD inode（`socket:[inode]`）负责把 socket 归因到 PID；条目转换后统一复用
//! core 的 `validate_socket_entry`，违规条目跳过并记诊断，不伪造端口。

use std::collections::HashSet;
use std::io;

use runquiry_core::{
    CapabilityStatus, DiagnosticCode, DiagnosticIssue, Inspection, NetworkInventory, OpenPortEntry,
    Pid, Port, Protocol, SocketEntry, validate_socket_entry,
};

use super::process::{LinuxPlatform, diagnostic_for_io};
use super::procfs::{InetSocketRow, UnixSocketRow, parse_inet_table, parse_unix_table};

/// 四张 inet 表的相对路径、协议与 IPv6 标记（witr `readSockets` 的表清单）。
const INET_TABLES: [(&str, Protocol, bool); 4] = [
    ("net/tcp", Protocol::Tcp, false),
    ("net/tcp6", Protocol::Tcp6, true),
    ("net/udp", Protocol::Udp, false),
    ("net/udp6", Protocol::Udp6, true),
];

/// 以 (pid, port, address, protocol, state) 去重后追加开放端口条目。
fn push_entry(
    seen: &mut HashSet<(Option<Pid>, Port, String, Protocol, String)>,
    ports: &mut Vec<OpenPortEntry>,
    pid: Option<Pid>,
    port: Port,
    entry: &SocketEntry,
) {
    let key = (
        pid,
        port,
        entry.address.clone(),
        entry.protocol,
        entry.state.clone(),
    );
    if seen.insert(key) {
        ports.push(OpenPortEntry {
            pid,
            port,
            address: entry.address.clone(),
            protocol: entry.protocol,
            state: entry.state.clone(),
        });
    }
}

impl LinuxPlatform {
    /// 读取单个文件；ENOENT（如 IPv6 关闭时无 tcp6）按「表不存在」处理。
    fn read_optional(&self, rel: &str) -> io::Result<Option<String>> {
        match self.procfs.read_string(rel) {
            Ok(content) => Ok(Some(content)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// 解析全部 inet 表为行记录；`any_readable` 区分「表都在但为空」与
    /// 「整体不可读」——后者不得以空集合冒充成功。
    fn inet_rows(&self) -> (Vec<(InetSocketRow, Protocol)>, Vec<DiagnosticIssue>, bool) {
        let mut rows = Vec::new();
        let mut issues = Vec::new();
        let mut any_readable = false;
        for (rel, protocol, ipv6) in INET_TABLES {
            match self.read_optional(rel) {
                Ok(Some(content)) => {
                    any_readable = true;
                    rows.extend(
                        parse_inet_table(&content, ipv6)
                            .into_iter()
                            .map(|row| (row, protocol)),
                    );
                }
                Ok(None) => {}
                Err(error) => issues.push(DiagnosticIssue::new(
                    if error.kind() == io::ErrorKind::PermissionDenied {
                        DiagnosticCode::PermissionDenied
                    } else {
                        DiagnosticCode::Unknown
                    },
                    format!("/proc/{rel} 不可读：{error}"),
                )),
            }
        }
        (rows, issues, any_readable)
    }

    /// 解析 Unix 表（`/proc/net/unix`）；读取失败只记诊断，不产生错误。
    fn unix_rows(&self) -> (Vec<UnixSocketRow>, Vec<DiagnosticIssue>) {
        match self.procfs.read_string("net/unix") {
            Ok(content) => (parse_unix_table(&content), Vec::new()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => (Vec::new(), Vec::new()),
            Err(error) => (
                Vec::new(),
                vec![DiagnosticIssue::new(
                    DiagnosticCode::Unknown,
                    format!("/proc/net/unix 不可读：{error}"),
                )],
            ),
        }
    }

    /// inet 行 → socket 条目：端口 0 / 非法端口记解析诊断并跳过，其余条目
    /// 必须通过 core 的 `validate_socket_entry`（平台真实输入复用统一边界
    /// 规则，fixture loader 同源）。
    fn inet_entry(
        row: &InetSocketRow,
        protocol: Protocol,
        owner_pid: Option<Pid>,
    ) -> Result<Option<SocketEntry>, String> {
        let port = Port::new(row.port_raw).map_err(|_| {
            format!(
                "{protocol:?} 表行 inode {} 端口 {} 非法（须为 1..=65535）",
                row.inode, row.port_raw
            )
        })?;
        let entry = SocketEntry {
            inode: Some(row.inode),
            port: Some(port),
            address: row.address.clone(),
            remote_addr: row.remote_addr.clone(),
            state: row.state.clone(),
            protocol,
            owner_pid,
        };
        validate_socket_entry(&entry)?;
        Ok(Some(entry))
    }

    /// Unix 行 → socket 条目（无端口语义，不得编造假端口）。
    fn unix_entry(
        row: &UnixSocketRow,
        owner_pid: Option<Pid>,
    ) -> Result<Option<SocketEntry>, String> {
        let entry = SocketEntry {
            inode: Some(row.inode),
            port: None,
            address: row.path.clone(),
            remote_addr: None,
            state: row.state.clone(),
            protocol: Protocol::Unix,
            owner_pid,
        };
        validate_socket_entry(&entry)?;
        Ok(Some(entry))
    }
}

impl NetworkInventory for LinuxPlatform {
    fn capability(&self) -> CapabilityStatus {
        CapabilityStatus::Supported
    }

    fn open_ports(&self) -> Inspection<Vec<OpenPortEntry>> {
        let (rows, mut issues, any_readable) = self.inet_rows();
        if !any_readable {
            // 全表不可读时不得以空集合冒充成功（部分成功红线）。
            if issues.is_empty() {
                issues.push(DiagnosticIssue::new(
                    DiagnosticCode::Unknown,
                    String::from("无任何 /proc/net 表可读，端口清单不可得"),
                ));
            }
            return Inspection::failed(issues);
        }
        let mut entries = Vec::new();
        for (row, protocol) in &rows {
            match Self::inet_entry(row, *protocol, None) {
                Ok(Some(entry)) => entries.push(entry),
                Ok(None) => {}
                Err(reason) => {
                    issues.push(DiagnosticIssue::new(DiagnosticCode::ParseFailed, reason));
                }
            }
        }
        let (owners, unreadable) = self.scan_fd_ownership();
        if unreadable > 0 {
            issues.push(DiagnosticIssue::new(
                DiagnosticCode::PermissionDenied,
                format!("{unreadable} 个进程的 /proc/PID/fd 不可读，其持有端口无法归因到 PID"),
            ));
        }
        let mut seen = HashSet::new();
        let mut ports = Vec::new();
        for entry in entries {
            let (Some(port), Some(inode)) = (entry.port, entry.inode) else {
                continue;
            };
            let candidates: &[Pid] = owners.get(&inode).map_or(&[][..], std::vec::Vec::as_slice);
            // 属主不可知（权限、退出或内核 socket）：条目保留为 None；
            // 有属主时按持有者展开（fork 共享 socket 会出现多个属主）。
            if candidates.is_empty() {
                push_entry(&mut seen, &mut ports, None, port, &entry);
            } else {
                for pid in candidates {
                    push_entry(&mut seen, &mut ports, Some(*pid), port, &entry);
                }
            }
        }
        if issues.is_empty() {
            Inspection::complete(ports)
        } else {
            Inspection::partial(ports, issues)
        }
    }

    fn sockets_of(&self, pid: Pid) -> Inspection<Vec<SocketEntry>> {
        let inodes = match self.fd_socket_inodes(pid.get()) {
            Ok(inodes) => inodes,
            Err(error) => return Inspection::failed(vec![diagnostic_for_io(pid.get(), &error)]),
        };
        let (inet_rows, mut issues, _) = self.inet_rows();
        let (unix_rows, unix_issues) = self.unix_rows();
        issues.extend(unix_issues);
        let mut seen = HashSet::new();
        let mut entries = Vec::new();
        for inode in inodes {
            for (row, protocol) in &inet_rows {
                if row.inode != inode || !seen.insert((inode, *protocol)) {
                    continue;
                }
                match Self::inet_entry(row, *protocol, Some(pid)) {
                    Ok(Some(entry)) => entries.push(entry),
                    Ok(None) => {}
                    Err(reason) => {
                        issues.push(DiagnosticIssue::new(DiagnosticCode::ParseFailed, reason));
                    }
                }
            }
            for row in &unix_rows {
                if row.inode != inode || !seen.insert((inode, Protocol::Unix)) {
                    continue;
                }
                match Self::unix_entry(row, Some(pid)) {
                    Ok(Some(entry)) => entries.push(entry),
                    Ok(None) => {}
                    Err(reason) => {
                        issues.push(DiagnosticIssue::new(DiagnosticCode::ParseFailed, reason));
                    }
                }
            }
        }
        if issues.is_empty() {
            Inspection::complete(entries)
        } else {
            Inspection::partial(entries, issues)
        }
    }
}
