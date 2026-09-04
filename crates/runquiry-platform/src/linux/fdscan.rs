//! FD → PID 归因扫描：`/proc/PID/fd` 的 socket inode 归因（网络适配器共用）。

use std::collections::HashMap;
use std::io;

use runquiry_core::Pid;

use super::process::LinuxPlatform;
use super::procfs::parse_socket_inode;

impl LinuxPlatform {
    /// 指定进程 FD 目录里的 socket inode 列表。
    pub(in crate::linux) fn fd_socket_inodes(&self, pid: u32) -> io::Result<Vec<u64>> {
        let mut inodes = Vec::new();
        for name in self.procfs.read_dir_names(&format!("{pid}/fd"))? {
            if let Ok(link) = self.procfs.read_link(&format!("{pid}/fd/{name}"))
                && let Some(inode) = parse_socket_inode(&link.to_string_lossy())
            {
                inodes.push(inode);
            }
        }
        Ok(inodes)
    }

    /// 全系统 FD → PID 归因扫描：inode → 持有者集合，以及 fd 目录不可读的
    /// 进程数（权限类诊断的依据；目录消失的进程静默跳过）。
    pub(in crate::linux) fn scan_fd_ownership(&self) -> (HashMap<u64, Vec<Pid>>, usize) {
        let mut owners: HashMap<u64, Vec<Pid>> = HashMap::new();
        let mut unreadable = 0usize;
        let Ok(pids) = self.procfs.list_pids() else {
            return (owners, unreadable);
        };
        for pid in pids {
            let Ok(pid) = Pid::new(pid) else { continue };
            match self.fd_socket_inodes(pid.get()) {
                Ok(inodes) => {
                    for inode in inodes {
                        owners.entry(inode).or_default().push(pid);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(_) => unreadable += 1,
            }
        }
        (owners, unreadable)
    }
}
