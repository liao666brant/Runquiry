//! Windows 平台采集适配器（Batch 6 C2，模块计划 07-windows-platform）。
//!
//! 仅在 `target_os = "windows"` 下参与编译（`lib.rs` 的 cfg 声明）；为 core
//! 端口提供真实实现：进程基线与详情（sysinfo + PEB 安全包装）、网络与
//! Socket（IP Helper）、来源证据（SCM）、容器归属验证、文件锁与进程操作
//! 的 Unsupported 建模。FFI 只存在于 `ffi` / `ffi_scm` 安全包装，纯解析
//! （utf16 / peb / ip_table / scm_parse / winerror / unsupported）无 OS 依赖，
//! 可在 Linux 编译并由 `tests/windows_*.rs` 经 `#[path]` 直接执行。
//!
//! # windows-sys 0.61.2 feature 清单（manifest 由主代理维护，两处须同步）
//!
//! - `Win32_Foundation`：HANDLE / CloseHandle / GetLastError /
//!   FILETIME / INVALID_HANDLE_VALUE / WIN32_ERROR 常量 / NTSTATUS
//! - `Win32_System_Threading`：OpenProcess、GetProcessTimes、
//!   GetProcessIoCounters、GetProcessHandleCount、IsWow64Process、
//!   QueryFullProcessImageNameW、PROCESS_* 权限常量、IO_COUNTERS
//! - `Win32_System_SystemInformation`：GlobalMemoryStatusEx、MEMORYSTATUSEX
//! - `Win32_System_Diagnostics_ToolHelp`：CreateToolhelp32Snapshot、
//!   Process32FirstW/NextW、PROCESSENTRY32W、TH32CS_SNAPPROCESS
//! - `Win32_System_Diagnostics_Debug`：ReadProcessMemory
//! - `Win32_System_ProcessStatus`：GetProcessMemoryInfo、
//!   PROCESS_MEMORY_COUNTERS(_EX)
//! - `Win32_NetworkManagement_IpHelper`：GetExtendedTcpTable /
//!   GetExtendedUdpTable、MIB_*_OWNER_PID 表、TCP/UDP_TABLE_CLASS
//! - `Win32_Networking_WinSock`：AF_INET / AF_INET6
//! - `Win32_System_Services`：SCM 枚举与配置查询（含 SERVICE_* 常量）
//! - `Wdk_System_Threading`：NtQueryInformationProcess（ProcessBasicInformation
//!   / ProcessWow64Information）
//!
//! 手写 extern：无——ntdll 函数经 `Wdk_System_Threading` feature 导出。

mod controller;
mod details;
mod ffi;
mod ffi_scm;
mod ip_table;
mod limits;
mod network;
mod peb;
mod peb_reader;
mod process_list;
mod scm_parse;
mod source;
mod unsupported;
mod utf16;
mod winerror;

use std::io;
use std::time::SystemTime;

use runquiry_core::Pid;

/// Windows 只读采集适配器：单一结构实现七个端口。
///
/// 自身排除策略与 [`LinuxPlatform`](crate::linux::LinuxPlatform) 对齐：
/// 构造时以 sysinfo 采集一次 PID 基准快照；`list()` 排除自身 PID，以及
/// 「不在基准快照中且（a）PPID 链可达自身或（b）启动晚于构造时刻」的
/// PID（构造后派生的短生命周期辅助进程）。
#[derive(Debug, Clone)]
pub struct WindowsPlatform {
    /// Runquiry 自身 PID（恒被排除）。
    own_pid: Pid,
    /// 构造时刻的 PID 基准快照。
    baseline_pids: Vec<u32>,
    /// 构造时刻（参与自身排除的时间窗规则）。
    constructed_at: SystemTime,
}

impl WindowsPlatform {
    /// 以生产配置构造：sysinfo PID 基准快照与真实墙钟。
    ///
    /// # Errors
    /// 自身 PID 为 0 时返回错误（正常进程不会发生）。
    pub fn new() -> io::Result<Self> {
        let own_pid = Pid::new(std::process::id())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "自身 PID 为 0"))?;
        Ok(Self {
            own_pid,
            baseline_pids: process_list::sysinfo_pids(),
            constructed_at: SystemTime::now(),
        })
    }

    /// 是否应从基线排除（策略同 LinuxPlatform::excluded）。
    pub(super) fn excluded(
        &self,
        pid: u32,
        start_time: Option<SystemTime>,
        parents: &std::collections::HashMap<u32, u32>,
    ) -> bool {
        if pid == self.own_pid.get() {
            return true;
        }
        if self.baseline_pids.contains(&pid) {
            return false;
        }
        if let Some(start) = start_time
            && start > self.constructed_at
        {
            return true;
        }
        // 沿 PPID 链上溯找自身；环与超长链以步数上限截断。
        let mut current = pid;
        for _ in 0..4_096 {
            match parents.get(&current) {
                Some(&parent) if parent == self.own_pid.get() => return true,
                Some(&parent) if parent != 0 => current = parent,
                _ => return false,
            }
        }
        false
    }
}
