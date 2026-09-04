//! Linux 进程基线与详情适配器：`ProcessInventory` / `ProcessDetailsProvider`
//! 的 `/proc` 真实只读实现。
//!
//! 进程枚举以 `/proc` 目录扫描为基线并经 procfs 补齐全部字段；sysinfo 提供
//! uid → 用户名解析（witr 的 `/etc/passwd` 手工解析等价物）。所有 `/proc`
//! 读取经可注入的 [`ProcFs`] 根目录，测试用合成树驱动同一条代码路径。

use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

use runquiry_core::{
    CapabilityStatus, ContainerContext, DiagnosticCode, DiagnosticIssue, HealthStatus, Inspection,
    Pid, ProcessIdentity, ProcessInventory, ProcessSummary,
};
use sysinfo::Users;

use super::capabilities::decode_capabilities;
use super::procfs::{
    CLK_TCK, PAGE_SIZE, ProcFs, StatInfo, parse_boot_time, parse_environ, parse_null_list,
    parse_stat, parse_status, start_time_from_ticks,
};

/// 单条目读取失败 → 诊断（部分成功红线：单点失败不抹掉其余条目）。
pub(super) fn diagnostic_for_io(pid: u32, error: &io::Error) -> DiagnosticIssue {
    if error.kind() == io::ErrorKind::PermissionDenied {
        DiagnosticIssue::new(
            DiagnosticCode::PermissionDenied,
            format!("进程 {pid} 的 /proc 条目不可读（权限不足）"),
        )
    } else {
        DiagnosticIssue::new(
            DiagnosticCode::Unknown,
            format!("进程 {pid} 在采集中消失或其 /proc 条目不可读：{error}"),
        )
    }
}

/// Linux 只读采集适配器：单一结构实现五个只读端口。
///
/// 自身排除策略（可测试，不靠进程名猜测）：构造时对 `/proc` 做一次 PID
/// 快照作为基准；`list()` 排除 Runquiry 自身 PID，以及「不在基准快照中且
/// PPID 链可达自身」的 PID——即采集开始后由自身派生的短生命周期辅助进程。
#[derive(Debug, Clone)]
pub struct LinuxPlatform {
    /// 可注入的 `/proc` 根。
    pub(super) procfs: ProcFs,
    /// Runquiry 自身 PID（恒被排除）。
    own_pid: Pid,
    /// 构造时刻的 PID 基准快照。
    baseline_pids: Vec<u32>,
    /// systemd 运行探测目录（parity：`IsSystemdRunning` 读
    /// `/run/systemd/system` 存在性）。
    pub(super) systemd_run_dir: PathBuf,
    /// 构造时刻（生产为墙钟；合成树测试经 [`Self::with_constructed_at`]
    /// 注入确定性值）。`Some` 时参与自身排除：启动晚于构造时刻的进程视为
    /// 采集期辅助进程，即使其父进程已退出（被收养导致 PPID 链断裂）。
    constructed_at: Option<SystemTime>,
}

impl LinuxPlatform {
    /// 以生产配置构造：`/proc` 真实根目录、自身 PID 与基准快照。
    ///
    /// # Errors
    /// 自身 PID 为 0 时返回错误（正常进程不会发生）。
    pub fn new() -> io::Result<Self> {
        let own_pid = Pid::new(std::process::id())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "自身 PID 为 0"))?;
        Ok(Self::with_injected(
            PathBuf::from("/proc"),
            PathBuf::from("/run/systemd/system"),
            own_pid,
        ))
    }

    /// 以显式注入的根目录、systemd 探测目录与自身 PID 构造（测试合成树与
    /// QA 使用）。基准快照在构造时采集；构造后再写入合成树的 PID 按
    /// 「采集开始后由自身派生」处理。
    #[must_use]
    pub fn with_injected(proc_root: PathBuf, systemd_run_dir: PathBuf, own_pid: Pid) -> Self {
        let procfs = ProcFs::new(proc_root);
        let baseline_pids = procfs.list_pids().unwrap_or_default();
        Self {
            procfs,
            own_pid,
            baseline_pids,
            systemd_run_dir,
            constructed_at: None,
        }
    }

    /// 注入构造时刻（测试用；启用「启动晚于构造时刻即排除」的时间窗规则）。
    #[must_use]
    pub const fn with_constructed_at(mut self, at: SystemTime) -> Self {
        self.constructed_at = Some(at);
        self
    }

    /// `/proc` 根目录（QA 示例展示用）。
    #[must_use]
    pub fn proc_root(&self) -> &std::path::Path {
        &self.procfs.root
    }

    pub(in crate::linux) fn boot_time(&self) -> Option<SystemTime> {
        let raw = self.procfs.read_string("stat").ok()?;
        parse_boot_time(&raw)
    }

    pub(super) fn stat_of(&self, pid: u32) -> io::Result<StatInfo> {
        let raw = self.procfs.read_string(&format!("{pid}/stat"))?;
        parse_stat(&raw).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("PID {pid} 的 stat 字段不足 22"),
            )
        })
    }

    /// exe 链接目标与「已删除」标记（witr `isBinaryDeleted`：只有 readlink
    /// 成功且目标带 " (deleted)" 后缀才置位；权限不足与进程消失不构成证据）。
    pub(super) fn exe_of(&self, pid: u32) -> (Option<PathBuf>, bool) {
        self.procfs
            .read_link(&format!("{pid}/exe"))
            .map_or((None, false), |target| {
                // 只有 readlink 成功且目标带后缀才置位；权限不足与进程消失不构成证据。
                let deleted = target.to_string_lossy().ends_with(" (deleted)");
                (Some(target), deleted)
            })
    }

    /// uid → 用户名（sysinfo 用户表；/etc/passwd 之外的 uid 回退为数字串，
    /// witr `readUser` 的兜底语义）。用户表由调用方一次采集、整轮 `list()`
    /// 复用（避免每进程重读 /etc/passwd）。
    pub(super) fn user_of(uid: Option<u32>, users: &Users) -> Option<String> {
        let uid = uid?;
        let sysinfo_uid = sysinfo::Uid::try_from(usize::try_from(uid).ok()?).ok();
        sysinfo_uid
            .and_then(|uid| users.get_user_by_id(&uid))
            .map_or_else(
                || Some(uid.to_string()),
                |user| Some(String::from(user.name())),
            )
    }

    /// 是否应从基线排除：自身；基准快照之后新出现且（a）PPID 链可达自身，
    /// 或（b）启动时刻晚于构造时刻（父进程已退出的收养后代，PPID 链断裂时
    /// 由时间窗兜底）。
    fn excluded(
        &self,
        pid: u32,
        stat: &StatInfo,
        boot: Option<SystemTime>,
        ppids: &HashMap<u32, u32>,
    ) -> bool {
        if pid == self.own_pid.get() {
            return true;
        }
        if self.baseline_pids.contains(&pid) {
            return false;
        }
        if let Some(construction) = self.constructed_at
            && let Some(start) = boot.and_then(|boot| start_time_from_ticks(boot, stat.start_ticks))
            && start > construction
        {
            return true;
        }
        // 沿 PPID 链上溯找自身；环与超长链以步数上限截断。
        let mut current = pid;
        for _ in 0..4_096 {
            match ppids.get(&current) {
                Some(&parent) if parent == self.own_pid.get() => return true,
                Some(&parent) if parent != 0 => current = parent,
                _ => return false,
            }
        }
        false
    }

    /// 全量快照的 PPID 映射（自身后代排除与子进程发现共用）。
    pub(super) fn ppid_map(&self) -> HashMap<u32, u32> {
        let Ok(pids) = self.procfs.list_pids() else {
            return HashMap::new();
        };
        pids.into_iter()
            .filter_map(|pid| self.stat_of(pid).ok().map(|stat| (pid, stat.ppid)))
            .collect()
    }

    fn cgroup_context_of(&self, pid: u32) -> Option<ContainerContext> {
        let raw = self.procfs.read_string(&format!("{pid}/cgroup")).ok()?;
        runquiry_core::detect_container_from_cgroup(&raw)
    }

    fn command_of(stat: &StatInfo, command_line: Option<&str>) -> String {
        if stat.comm.is_empty() {
            // comm 缺失时回退命令行首参数的末段。
            command_line
                .and_then(|line| line.split_whitespace().next())
                .and_then(|first| first.rsplit('/').next())
                .unwrap_or_default()
                .to_string()
        } else {
            stat.comm.clone()
        }
    }

    fn build_summary(
        &self,
        pid: u32,
        stat: &StatInfo,
        boot: Option<SystemTime>,
        users: &Users,
    ) -> ProcessSummary {
        let status = self
            .procfs
            .read_string(&format!("{pid}/status"))
            .map(|raw| parse_status(&raw))
            .unwrap_or_default();
        let command_line = self
            .procfs
            .read(&format!("{pid}/cmdline"))
            .ok()
            .map(|bytes| parse_null_list(&bytes).join(" "))
            .filter(|line| !line.is_empty());
        let (exe, exe_deleted) = self.exe_of(pid);
        let start_time = boot.and_then(|boot| start_time_from_ticks(boot, stat.start_ticks));
        let health = match stat.state {
            'Z' => HealthStatus::Zombie,
            'T' => HealthStatus::Stopped,
            // parity witr process_linux.go:193-200：healthy 时才判 high-cpu
            // （累计 CPU > 2h）与 high-mem（RSS > 1GiB），前者优先。
            _ if (stat.utime + stat.stime) / CLK_TCK > 2 * 60 * 60 => HealthStatus::HighCpu,
            _ if stat.rss_pages.saturating_mul(PAGE_SIZE) > 1024 * 1024 * 1024 => {
                HealthStatus::HighMem
            }
            _ => HealthStatus::Healthy,
        };
        ProcessSummary {
            identity: ProcessIdentity::new(Pid::new(pid).unwrap_or(Pid::MIN), start_time, exe),
            parent_pid: Pid::new(stat.ppid).ok(),
            command: Self::command_of(stat, command_line.as_deref()),
            command_line,
            user: Self::user_of(status.uid, users),
            health,
            container: self.cgroup_context_of(pid),
            exe_deleted,
            capabilities: status
                .cap_eff_hex
                .as_deref()
                .map(decode_capabilities)
                .unwrap_or_default(),
        }
    }
}

impl ProcessInventory for LinuxPlatform {
    fn capability(&self) -> CapabilityStatus {
        CapabilityStatus::Supported
    }

    fn list(&self) -> Inspection<Vec<ProcessSummary>> {
        let pids = match self.procfs.list_pids() {
            Ok(pids) => pids,
            Err(error) => {
                return Inspection::failed(vec![DiagnosticIssue::new(
                    DiagnosticCode::Unknown,
                    format!("/proc 不可枚举：{error}"),
                )]);
            }
        };
        let boot = self.boot_time();
        let users = Users::new_with_refreshed_list();
        let baseline_ppids = self.ppid_map();
        let mut summaries = Vec::new();
        let mut issues = Vec::new();
        for pid in pids {
            match self.stat_of(pid) {
                Ok(stat) => {
                    if !self.excluded(pid, &stat, boot, &baseline_ppids) {
                        summaries.push(self.build_summary(pid, &stat, boot, &users));
                    }
                }
                Err(error) => issues.push(diagnostic_for_io(pid, &error)),
            }
        }
        if summaries.is_empty() && !issues.is_empty() {
            return Inspection::failed(issues);
        }
        if issues.is_empty() {
            Inspection::complete(summaries)
        } else {
            Inspection::partial(summaries, issues)
        }
    }
}

/// 读取 environ 键值（详情与来源证据共用；值不写入任何日志）。
pub(super) fn read_environ(procfs: &ProcFs, pid: u32) -> Vec<(String, String)> {
    procfs
        .read(&format!("{pid}/environ"))
        .map(|bytes| parse_environ(&bytes))
        .unwrap_or_default()
}

/// 读取 cgroup 原文（来源证据采集）。
pub(super) fn read_cgroup(procfs: &ProcFs, pid: u32) -> Option<String> {
    procfs.read_string(&format!("{pid}/cgroup")).ok()
}
