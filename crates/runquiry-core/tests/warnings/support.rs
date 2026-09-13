//! 告警测试共享合成输入。

use std::time::{Duration, SystemTime};

use runquiry_core::{
    HealthStatus, Pid, ProcessIdentity, ProcessSummary, Protocol, SocketEntry, Warning, WarningKind,
};

/// 返回默认无告警条件的合成目标进程。
pub fn target() -> ProcessSummary {
    let identity = ProcessIdentity::new(
        Pid::new(5).unwrap_or(Pid::MIN),
        Some(SystemTime::UNIX_EPOCH + Duration::from_hours(1)),
        None,
    );
    ProcessSummary {
        identity,
        parent_pid: None,
        command: String::from("fxt-daemon"),
        command_line: None,
        user: Some(String::from("fixture-user")),
        health: HealthStatus::Unknown,
        container: None,
        exe_deleted: false,
        capabilities: Vec::new(),
        cpu_time_seconds: None,
        cpu_percent: None,
        memory_rss_bytes: None,
        memory_percent: None,
    }
}

/// 返回指定地址与状态的合成 TCP socket。
pub fn listen_socket(address: &str, state: &str) -> SocketEntry {
    SocketEntry {
        inode: None,
        port: runquiry_core::Port::new(8443).ok(),
        address: String::from(address),
        remote_addr: None,
        state: String::from(state),
        protocol: Protocol::Tcp,
        owner_pid: Some(Pid::new(5).unwrap_or(Pid::MIN)),
    }
}

/// 提取告警的稳定类型序列。
pub fn kinds(warnings: &[Warning]) -> Vec<WarningKind> {
    warnings.iter().map(Warning::kind).collect()
}
