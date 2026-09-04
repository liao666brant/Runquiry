//! Linux 进程详情适配器：硬身份失败与字段级部分成功。

use std::io;

use runquiry_core::{
    CapabilityStatus, DiagnosticCode, DiagnosticIssue, InspectError, Inspection, Pid,
    ProcessDetails, ProcessDetailsProvider, ProcessIdentity,
};

use super::process::LinuxPlatform;
use super::procfs::{
    parse_environ_checked, parse_fd_limit, parse_io, parse_meminfo_total, parse_statm,
    start_time_from_ticks,
};

struct PartialDetails {
    pid: u32,
    issues: Vec<DiagnosticIssue>,
}

impl PartialDetails {
    const fn new(pid: u32) -> Self {
        Self {
            pid,
            issues: Vec::new(),
        }
    }

    fn io<T>(&mut self, field: &str, result: io::Result<T>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                let code = if error.kind() == io::ErrorKind::PermissionDenied {
                    DiagnosticCode::PermissionDenied
                } else {
                    DiagnosticCode::Unknown
                };
                self.issues.push(DiagnosticIssue::new(
                    code,
                    format!("进程 {} 的 {field} 不可读：{error}", self.pid),
                ));
                None
            }
        }
    }

    fn parsed<T>(&mut self, field: &str, value: Option<T>) -> Option<T> {
        if value.is_none() {
            self.parse_failed(field);
        }
        value
    }

    fn parse_failed(&mut self, field: &str) {
        self.issues.push(DiagnosticIssue::new(
            DiagnosticCode::ParseFailed,
            format!("进程 {} 的 {field} 无法解析", self.pid),
        ));
    }

    fn finish(self, details: ProcessDetails) -> Inspection<ProcessDetails> {
        if self.issues.is_empty() {
            Inspection::complete(details)
        } else {
            Inspection::partial(details, self.issues)
        }
    }
}

impl ProcessDetailsProvider for LinuxPlatform {
    fn capability(&self) -> CapabilityStatus {
        CapabilityStatus::Supported
    }

    fn details(
        &self,
        identity: &ProcessIdentity,
    ) -> Result<Inspection<ProcessDetails>, InspectError> {
        let pid = identity.pid().get();
        let subject = format!("进程 {pid}");
        let stat = self.stat_of(pid).map_err(|error| match error.kind() {
            io::ErrorKind::PermissionDenied => InspectError::PermissionDenied { subject },
            _ => InspectError::NotFound { subject },
        })?;
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

        let mut partial = PartialDetails::new(pid);
        let exe = partial.io("exe", self.procfs.read_link(&format!("{pid}/exe")));
        let current_identity = ProcessIdentity::new(identity.pid(), current_start, exe);
        let environment = partial
            .io("environ", self.procfs.read(&format!("{pid}/environ")))
            .map_or_else(Vec::new, |bytes| {
                let (environment, valid) = parse_environ_checked(&bytes);
                if !valid {
                    partial.parse_failed("environ");
                }
                environment
            });
        let working_dir = partial.io("cwd", self.procfs.read_link(&format!("{pid}/cwd")));
        let statm = partial
            .io("statm", self.procfs.read_string(&format!("{pid}/statm")))
            .and_then(|raw| partial.parsed("statm", parse_statm(&raw)));
        let memory_rss_bytes = statm.as_ref().map(|memory| memory.rss_bytes);
        let total_memory = partial
            .io("meminfo", self.procfs.read_string("meminfo"))
            .and_then(|raw| partial.parsed("meminfo", parse_meminfo_total(&raw)));
        #[allow(clippy::cast_precision_loss)]
        let memory_percent = match (memory_rss_bytes, total_memory) {
            (Some(rss), Some(total)) if total > 0 => Some(rss as f64 / total as f64 * 100.0),
            _ => None,
        };
        let io = partial
            .io("io", self.procfs.read_string(&format!("{pid}/io")))
            .and_then(|raw| partial.parsed("io", parse_io(&raw)));
        let (open_files, fd_count) = self.collect_fds(pid, &mut partial);
        let fd_limit = partial
            .io("limits", self.procfs.read_string(&format!("{pid}/limits")))
            .and_then(|raw| partial.parsed("limits", parse_fd_limit(&raw)));
        let children = self.collect_children(pid, &mut partial);

        Ok(partial.finish(ProcessDetails {
            identity: current_identity,
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
        }))
    }
}

impl LinuxPlatform {
    fn collect_fds(
        &self,
        pid: u32,
        partial: &mut PartialDetails,
    ) -> (Vec<std::path::PathBuf>, Option<u64>) {
        let Some(names) = partial.io("fd", self.procfs.read_dir_names(&format!("{pid}/fd"))) else {
            return (Vec::new(), None);
        };
        let count = Some(u64::try_from(names.len()).unwrap_or(u64::MAX));
        let files = names
            .into_iter()
            .filter_map(|name| {
                partial.io(
                    "fd entry",
                    self.procfs.read_link(&format!("{pid}/fd/{name}")),
                )
            })
            .collect();
        (files, count)
    }

    fn collect_children(&self, pid: u32, partial: &mut PartialDetails) -> Vec<Pid> {
        let Some(candidates) = partial.io("children", self.procfs.list_pids()) else {
            return Vec::new();
        };
        candidates
            .into_iter()
            .filter_map(|candidate| match self.stat_of(candidate) {
                Ok(stat) if stat.ppid == pid => Pid::new(candidate).ok(),
                Ok(_) => None,
                Err(error) => {
                    let _ignored = partial.io::<()>("children stat", Err(error));
                    None
                }
            })
            .collect()
    }
}
