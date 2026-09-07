//! macOS 网络与 Socket 采集（`NetworkInventory` 的 lsof 实现）。
//!
//! 语义对齐 witr `net_darwin.go::ListOpenPorts`（`lsof -i -P -n` 列格式；
//! `socketsForPID` 从全量端口清单按 PID 过滤去重）。部分成功语义：
//! * lsof 缺失 / 超时 / 超限 → [`InspectError::ExternalTool`]（整体失败）；
//! * 非零退出但有部分 stdout → 保留可解析条目 + 抢救诊断（对齐
//!   `macos/open-ports-timeout` fixture 的部分成功语义）；
//! * 属主不可知行 → `pid: None` 条目 + 诊断（core 端口后置条件；witr 静默
//!   跳过，此处如实保留条目以免丢端口）。

use std::collections::HashSet;

use runquiry_core::{
    CapabilityStatus, DiagnosticCode, DiagnosticIssue, Inspection, LIST_TIMEOUT, NetworkInventory,
    OpenPortEntry, Pid, Port, SocketEntry, validate_socket_entry,
};

use super::MacosPlatform;
use super::lsof;

impl NetworkInventory for MacosPlatform {
    fn capability(&self) -> CapabilityStatus {
        CapabilityStatus::Partial(String::from(
            "lsof best-effort：非 root 下其他用户进程的端口不可归因（条目保留为无属主）",
        ))
    }

    fn open_ports(&self) -> Inspection<Vec<OpenPortEntry>> {
        let output = match self.run_lsof(&["-i", "-P", "-n"], LIST_TIMEOUT) {
            Ok(output) => output,
            Err(error) => {
                return Inspection::failed(vec![DiagnosticIssue::new(
                    DiagnosticCode::ExternalToolFailed,
                    format!("lsof 不可用或超时：{error}"),
                )]);
            }
        };
        let mut issues = Vec::new();
        if let Some(issue) = super::exit_salvage_issue(&output, "lsof -i -P -n") {
            issues.push(issue);
        }
        let (rows, parse_issues) = lsof::parse_open_ports(&String::from_utf8_lossy(&output.stdout));
        issues.extend(
            parse_issues
                .into_iter()
                .map(|reason| DiagnosticIssue::new(DiagnosticCode::ParseFailed, reason)),
        );
        let mut seen = HashSet::new();
        let mut ports = Vec::new();
        for row in rows {
            let Ok(port) = Port::new(row.port) else {
                issues.push(DiagnosticIssue::new(
                    DiagnosticCode::ParseFailed,
                    format!("lsof -i 行端口 {} 非法，跳过", row.port),
                ));
                continue;
            };
            let pid = row.pid.and_then(|pid| Pid::new(pid).ok());
            let entry = OpenPortEntry {
                pid,
                port,
                address: row.address.clone(),
                protocol: row.protocol,
                state: row.state.clone(),
            };
            if seen.insert((
                pid,
                port,
                entry.address.clone(),
                entry.protocol,
                entry.state.clone(),
            )) {
                ports.push(entry);
            }
        }
        if issues.is_empty() {
            Inspection::complete(ports)
        } else {
            Inspection::partial(ports, issues)
        }
    }

    fn sockets_of(&self, pid: Pid) -> Inspection<Vec<SocketEntry>> {
        // witr `socketsForPID`：从全量端口清单按 PID 过滤（单次 lsof，复用
        // open_ports 的解析与部分成功语义）。
        let listed = self.open_ports();
        let mut issues = listed.issues;
        let sockets = listed
            .data
            .unwrap_or_default()
            .into_iter()
            .filter(|entry| entry.pid == Some(pid))
            .map(|entry| SocketEntry {
                inode: None,
                port: Some(entry.port),
                address: entry.address,
                remote_addr: None,
                state: entry.state,
                protocol: entry.protocol,
                owner_pid: Some(pid),
            })
            .filter(|entry| match validate_socket_entry(entry) {
                Ok(()) => true,
                Err(reason) => {
                    issues.push(DiagnosticIssue::new(DiagnosticCode::ParseFailed, reason));
                    false
                }
            })
            .collect::<Vec<_>>();
        let mut seen = HashSet::new();
        let sockets: Vec<SocketEntry> = sockets
            .into_iter()
            .filter(|entry| {
                seen.insert((
                    entry.port,
                    entry.address.clone(),
                    entry.protocol,
                    entry.state.clone(),
                ))
            })
            .collect();
        if issues.is_empty() {
            Inspection::complete(sockets)
        } else {
            Inspection::partial(sockets, issues)
        }
    }
}
