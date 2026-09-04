//! `/proc` 只读采集：可注入根目录与原始文本解析。
//!
//! 所有 `/proc` 读取都经由以 [`ProcFs::root`] 为前缀的相对路径访问：生产实例
//! 根目录为 `/proc`，测试与 QA 用合成树（tempdir）替换根目录即可完整驱动
//! 采集路径，不依赖真实内核状态。解析函数均为纯函数，语义与 witr 的
//! `process_linux.go` / `net_linux.go` / `extended_linux.go` /
//! `filecontext_linux.go` / `boot_linux.go` 逐条对齐。

use std::fs;
use std::io;
use std::path::PathBuf;

/// `/proc` 根目录抽象：生产为 `/proc`，测试为合成树根。
#[derive(Debug, Clone)]
pub(super) struct ProcFs {
    /// `/proc` 根目录（生产 `/proc`；测试指向 tempdir 合成树）。
    pub(super) root: PathBuf,
}

impl ProcFs {
    /// 以给定根目录构造。
    pub(super) const fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// 拼接 `/proc` 内相对路径。
    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    /// 读取一个小文件（进程消失、权限不足等按 `io::Error` 上抛）。
    pub(super) fn read(&self, rel: &str) -> io::Result<Vec<u8>> {
        fs::read(self.path(rel))
    }

    /// 读取一个小文件为字符串（保留原文，含结尾换行）。
    pub(super) fn read_string(&self, rel: &str) -> io::Result<String> {
        String::from_utf8(fs::read(self.path(rel))?)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "非 UTF-8 的 proc 文件"))
    }

    /// 读取符号链接目标（cwd / exe / fd）。
    pub(super) fn read_link(&self, rel: &str) -> io::Result<PathBuf> {
        fs::read_link(self.path(rel))
    }

    /// 列出目录条目名。
    pub(super) fn read_dir_names(&self, rel: &str) -> io::Result<Vec<String>> {
        let mut names = Vec::new();
        for entry in fs::read_dir(self.path(rel))? {
            let entry = entry?;
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
        Ok(names)
    }

    /// 枚举数字 PID 目录（按 PID 升序；非数字条目跳过）。
    pub(super) fn list_pids(&self) -> io::Result<Vec<u32>> {
        let mut pids = Vec::new();
        for name in self.read_dir_names("")? {
            if let Ok(pid) = name.parse::<u32>() {
                pids.push(pid);
            }
        }
        pids.sort_unstable();
        Ok(pids)
    }
}

/// `/proc/PID/stat` 解析结果（parity：PPID、state、starttime、CPU ticks、RSS）。
mod locktable;
mod netparse;
mod process_files;

pub(super) use locktable::parse_locks;
pub(super) use netparse::{
    InetSocketRow, UnixSocketRow, parse_inet_table, parse_socket_inode, parse_unix_table,
};
pub(super) use process_files::{
    CLK_TCK, PAGE_SIZE, StatInfo, StatusInfo, parse_boot_time, parse_environ,
    parse_environ_checked, parse_fd_limit, parse_io, parse_meminfo_total, parse_null_list,
    parse_stat, parse_statm, parse_status, start_time_from_ticks,
};
