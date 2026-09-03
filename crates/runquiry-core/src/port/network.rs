//! 网络与 Socket 采集端口。

use serde::{Deserialize, Serialize};

use crate::model::capability::CapabilityStatus;
use crate::model::ids::{Pid, Port};
use crate::model::inspection::Inspection;

/// 协议类型（socket 条目与开放端口条目共用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Protocol {
    /// IPv4 TCP。
    Tcp,
    /// IPv6 TCP。
    Tcp6,
    /// IPv4 UDP。
    Udp,
    /// IPv6 UDP。
    Udp6,
    /// Unix domain socket（路径挂在 [`SocketEntry::address`] 上，无端口号语义）。
    Unix,
}

/// 进程持有的 Socket 条目（parity：`Socket{Inode, Port, Address, State, Protocol}`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SocketEntry {
    /// 关联 inode；仅 Linux 有意义，其他平台为 `None`。
    pub inode: Option<u64>,
    /// 本地端口；仅 TCP / UDP 条目携带（parity：invalid port must be between 1
    /// and 65535 只约束端口语义）。Unix domain socket（[`Protocol::Unix`]）没有
    /// 端口语义，为 `None`——不得为满足类型而编造假端口。
    ///
    /// 带 `#[serde(default)]`：缺少该字段的旧序列化条目按 `None` 读取。
    #[serde(default)]
    pub port: Option<Port>,
    /// 本地地址（原样字符串，兼容 IPv4 / IPv6 / Unix socket 路径）。
    pub address: String,
    /// 已知属主连接的对端地址（parity：`SocketInfo.RemoteAddr`）；
    /// 监听 socket、无连接协议或对端不可得时为 `None`。
    ///
    /// 带 `#[serde(default)]`：缺少该字段的旧序列化条目按 `None` 读取。
    #[serde(default)]
    pub remote_addr: Option<String>,
    /// Socket 状态名（平台原样，如 `LISTEN`、`ESTAB`）。
    pub state: String,
    /// 协议。
    pub protocol: Protocol,
    /// 已归因的属主进程；属主不可知时为 `None`。
    pub owner_pid: Option<Pid>,
}

/// 开放端口条目（parity：`OpenPort{PID, Port, Address, Protocol, State}`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenPortEntry {
    /// 归因到的进程；属主不可知（无权限或已退出）时为 `None`。
    pub pid: Option<Pid>,
    /// 端口号。
    pub port: Port,
    /// 监听或连接地址。
    pub address: String,
    /// 协议。
    pub protocol: Protocol,
    /// Socket 状态名。
    pub state: String,
}

/// 网络与端口采集端口。
///
/// 后置条件：属主不可知的端口必须以 `pid: None` 返回条目，同时按约定追加
/// 权限类 [`DiagnosticIssue`](crate::model::diagnostic::DiagnosticIssue)；
/// 实现不得静默丢弃无主端口，也不得伪造属主。
pub trait NetworkInventory {
    /// 该能力的平台可用状态。
    fn capability(&self) -> CapabilityStatus;

    /// 采集当前全部开放端口条目。
    fn open_ports(&self) -> Inspection<Vec<OpenPortEntry>>;

    /// 采集指定进程持有的全部 Socket。
    fn sockets_of(&self, pid: Pid) -> Inspection<Vec<SocketEntry>>;
}
