//! Linux 容器宿主 PID 的 cgroup 归属验证。

use runquiry_core::{ContainerKey, ContainerProcessVerifier, Pid, verified_host_pid};

use super::LinuxPlatform;

impl ContainerProcessVerifier for LinuxPlatform {
    fn belongs_to_container(&self, pid: Pid, key: &ContainerKey) -> bool {
        let cgroup = self
            .procfs
            .read_string(&format!("{}/cgroup", pid.get()))
            .ok();
        verified_host_pid(Some(pid), key, cgroup.as_deref()).is_some()
    }
}
