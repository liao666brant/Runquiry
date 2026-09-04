//! Linux 进程详情适配器：`ProcessDetailsProvider` 的 `/proc` 真实只读实现。
//!
//! 与基线（[`super::process`]）共享 [`LinuxPlatform`]；PID 复用防护、扩展信息
//! （内存/I/O/FD/上限/子进程）与权限降级语义集中在此。

use std::io;

use runquiry_core::{
    CapabilityStatus, InspectError, Pid, ProcessDetails, ProcessDetailsProvider, ProcessIdentity,
};

use super::process::{LinuxPlatform, read_environ};
use super::procfs::{
    parse_fd_limit, parse_io, parse_meminfo_total, parse_statm, start_time_from_ticks,
};

impl ProcessDetailsProvider for LinuxPlatform {
    fn capability(&self) -> CapabilityStatus {
        CapabilityStatus::Supported
    }

    fn details(&self, identity: &ProcessIdentity) -> Result<ProcessDetails, InspectError> {
        let pid = identity.pid().get();
        let subject = format!("进程 {pid}");
        let stat = self.stat_of(pid).map_err(|error| match error.kind() {
            io::ErrorKind::PermissionDenied => InspectError::PermissionDenied { subject },
            // 进程消失与其他不可读（含 stat 格式损坏）统一视为目标不存在。
            _ => InspectError::NotFound { subject },
        })?;
        // PID 复用防护：调用方携带的启动时间与当前不一致即拒绝返回旧数据
        // （parity：`pidIdentityChanged` 比较 PID + StartedAt）。
        let current_start = self
            .boot_time()
            .and_then(|boot| start_time_from_ticks(boot, stat.start_ticks));
        if let (Some(expected), Some(current)) = (identity.start_time(), current_start)
            && expected != current
        {
            let (exe, _) = self.exe_of(pid);
            return Err(InspectError::ProcessChanged {
                identity: ProcessIdentity::new(identity.pid(), Some(current), exe),
            });
        }
        let (exe, _) = self.exe_of(pid);
        let identity = ProcessIdentity::new(identity.pid(), current_start, exe);
        let environment = read_environ(&self.procfs, pid);
        let working_dir = self.procfs.read_link(&format!("{pid}/cwd")).ok();
        let statm = self
            .procfs
            .read_string(&format!("{pid}/statm"))
            .ok()
            .and_then(|raw| parse_statm(&raw));
        let memory_rss_bytes = statm.as_ref().map(|mem| mem.rss_bytes);
        let total = self
            .procfs
            .read_string("meminfo")
            .ok()
            .and_then(|raw| parse_meminfo_total(&raw));
        #[allow(clippy::cast_precision_loss)] // RSS/总量换算百分比，u64→f64 精度损失可接受
        let memory_percent = match (memory_rss_bytes, total) {
            (Some(rss), Some(total)) if total > 0 => Some(rss as f64 / total as f64 * 100.0),
            _ => None,
        };
        let io = self
            .procfs
            .read_string(&format!("{pid}/io"))
            .ok()
            .map(|raw| parse_io(&raw));
        let fd_names = self.procfs.read_dir_names(&format!("{pid}/fd")).ok();
        let fd_count = fd_names
            .as_ref()
            .map(|names| u64::try_from(names.len()).unwrap_or(u64::MAX));
        let mut open_files = Vec::new();
        if let Some(names) = &fd_names {
            for name in names {
                if let Ok(target) = self.procfs.read_link(&format!("{pid}/fd/{name}")) {
                    open_files.push(target);
                }
            }
        }
        let fd_limit = self
            .procfs
            .read_string(&format!("{pid}/limits"))
            .ok()
            .and_then(|raw| parse_fd_limit(&raw));
        let child_ppids = self.ppid_map();
        let children = self
            .procfs
            .list_pids()
            .unwrap_or_default()
            .into_iter()
            .filter(|candidate| child_ppids.get(candidate) == Some(&pid))
            .filter_map(|candidate| Pid::new(candidate).ok())
            .collect();
        Ok(ProcessDetails {
            identity,
            cpu_percent: None,
            memory_rss_bytes,
            memory_percent,
            working_dir,
            environment,
            children,
            memory: statm,
            io,
            open_files,
            fd_count,
            fd_limit,
        })
    }
}
