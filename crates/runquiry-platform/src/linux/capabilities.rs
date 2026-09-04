//! Linux capabilities 位译码（witr `capabilities_linux.go` 的位名单与语义）。

/// 位位置 → capability 名（`include/uapi/linux/capability.h`，witr `capNames`）。
const CAP_NAMES: [&str; 41] = [
    "CAP_CHOWN",
    "CAP_DAC_OVERRIDE",
    "CAP_DAC_READ_SEARCH",
    "CAP_FOWNER",
    "CAP_FSETID",
    "CAP_KILL",
    "CAP_SETGID",
    "CAP_SETUID",
    "CAP_SETPCAP",
    "CAP_LINUX_IMMUTABLE",
    "CAP_NET_BIND_SERVICE",
    "CAP_NET_BROADCAST",
    "CAP_NET_ADMIN",
    "CAP_NET_RAW",
    "CAP_IPC_LOCK",
    "CAP_IPC_OWNER",
    "CAP_SYS_MODULE",
    "CAP_SYS_RAWIO",
    "CAP_SYS_CHROOT",
    "CAP_SYS_PTRACE",
    "CAP_SYS_PACCT",
    "CAP_SYS_ADMIN",
    "CAP_SYS_BOOT",
    "CAP_SYS_NICE",
    "CAP_SYS_RESOURCE",
    "CAP_SYS_TIME",
    "CAP_SYS_TTY_CONFIG",
    "CAP_MKNOD",
    "CAP_LEASE",
    "CAP_AUDIT_WRITE",
    "CAP_AUDIT_CONTROL",
    "CAP_SETFCAP",
    "CAP_MAC_OVERRIDE",
    "CAP_MAC_ADMIN",
    "CAP_SYSLOG",
    "CAP_WAKE_ALARM",
    "CAP_BLOCK_SUSPEND",
    "CAP_AUDIT_READ",
    "CAP_PERFMON",
    "CAP_BPF",
    "CAP_CHECKPOINT_RESTORE",
];

/// 把 `/proc/PID/status` 的 `CapEff` 十六进制位掩码译码为 capability 名单
/// （witr `decodeCapabilities`：空掩码返回空表；超出已知名单的位忽略）。
#[must_use]
pub(super) fn decode_capabilities(hex: &str) -> Vec<String> {
    let Ok(bits) = u64::from_str_radix(hex, 16) else {
        return Vec::new();
    };
    if bits == 0 {
        return Vec::new();
    }
    (0u32..)
        .take(CAP_NAMES.len())
        .filter(|bit| bits & (1 << bit) != 0)
        .map(|bit| String::from(CAP_NAMES[bit as usize]))
        .collect()
}
