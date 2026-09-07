//! macOS 容器宿主 PID 归属验证（`ContainerProcessVerifier`）。

use runquiry_core::{ContainerKey, ContainerProcessVerifier, Pid};

use super::MacosPlatform;

impl ContainerProcessVerifier for MacosPlatform {
    /// macOS 无 cgroup 证据：恒 `false`（trait 契约：证据缺失返回 `false`）。
    ///
    /// Docker Desktop / OrbStack 的容器进程运行在 Linux 虚拟机内，其 PID 与
    /// 宿主 macOS 的 PID 空间互不可见，运行时报告的「host PID」不可映射、
    /// 不得用于调查或控制。
    fn belongs_to_container(&self, _pid: Pid, _key: &ContainerKey) -> bool {
        false
    }
}