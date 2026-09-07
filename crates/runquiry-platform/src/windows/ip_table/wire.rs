//! IP Helper 表结构与行布局（纯逻辑：字段偏移与解码，不做整表解析）。

use runquiry_core::{Port, Protocol};

/// 表头之后的单行字节数（MIB_TCPROW_OWNER_PID：6 × u32）。
const TCP_V4_ROW_BYTES: usize = 24;
/// MIB_TCP6ROW_OWNER_PID：16 + 4 + 4 + 16 + 4 + 4 + 4 + 4 字节。
const TCP_V6_ROW_BYTES: usize = 56;
/// MIB_UDPROW_OWNER_PID：3 × u32。
const UDP_V4_ROW_BYTES: usize = 12;
/// MIB_UDP6ROW_OWNER_PID：16 + 4 + 4 + 4 字节。
const UDP_V6_ROW_BYTES: usize = 28;

/// 四类表（协议 = 行解码后的 core `Protocol`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TableKind {
    /// IPv4 TCP（`TCP_TABLE_OWNER_PID_ALL`，含监听行）。
    TcpV4,
    /// IPv6 TCP。
    TcpV6,
    /// IPv4 UDP（`UDP_TABLE_OWNER_PID`）。
    UdpV4,
    /// IPv6 UDP。
    UdpV6,
}

impl TableKind {
    /// 对应的 core 协议。
    #[must_use]
    pub(crate) const fn protocol(self) -> Protocol {
        match self {
            Self::TcpV4 => Protocol::Tcp,
            Self::TcpV6 => Protocol::Tcp6,
            Self::UdpV4 => Protocol::Udp,
            Self::UdpV6 => Protocol::Udp6,
        }
    }

    /// 表头后的单行字节数。
    #[must_use]
    pub(crate) const fn row_bytes(self) -> usize {
        match self {
            Self::TcpV4 => TCP_V4_ROW_BYTES,
            Self::TcpV6 => TCP_V6_ROW_BYTES,
            Self::UdpV4 => UDP_V4_ROW_BYTES,
            Self::UdpV6 => UDP_V6_ROW_BYTES,
        }
    }
}

/// 一行解码后的条目（地址为规范字符串，端口保留原始 DWORD 待校验）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SocketRow {
    /// 表类型（决定协议与状态语义）。
    pub kind: TableKind,
    /// 本地地址规范字符串（IPv6 为 RFC 5952 压缩形式）。
    pub address: String,
    /// 原始端口 DWORD（网络字节序，低 16 位有效）。
    pub port_raw: u32,
    /// 已知属主连接的对端（地址规范字符串, 原始端口 DWORD）。
    pub remote: Option<(String, u32)>,
    /// 状态名（TCP 映射 MIB_TCP_STATE；UDP 恒 `OPEN`）。
    pub state: String,
    /// 原始属主 PID（0 表示无属主）。
    pub owner_pid_raw: u32,
}

/// 将 MIB_TCP_STATE 数值映射为状态名（netstat 风格；`LISTENING` → `LISTEN`，
/// 与 witr `net_windows.go` 的 `LISTENING` 归一一致）。
#[must_use]
pub(crate) const fn tcp_state_name(state_raw: u32) -> &'static str {
    match state_raw {
        1 => "CLOSED",
        2 => "LISTEN",
        3 => "SYN_SENT",
        4 => "SYN_RCVD",
        5 => "ESTABLISHED",
        6 => "FIN_WAIT1",
        7 => "FIN_WAIT2",
        8 => "CLOSE_WAIT",
        9 => "CLOSING",
        10 => "LAST_ACK",
        11 => "TIME_WAIT",
        12 => "DELETE_TCB",
        100 => "RESERVED",
        _ => "UNKNOWN",
    }
}

/// UDP 条目状态名（witr `GetSocketsForPID` 语义：UDP 无状态，记 `OPEN`）。
pub(crate) const UDP_STATE: &str = "OPEN";

/// 解码网络字节序的端口 DWORD（低 16 位）；0 视为无端口（调用方跳过该行）。
#[must_use]
pub(crate) fn decode_port(port_raw: u32) -> Option<Port> {
    let value = u16::from_be((port_raw & 0xFFFF) as u16);
    Port::new(value).ok()
}

/// IPv4 地址：网络字节序 4 字节 → 规范点分十进制。
#[must_use]
pub(crate) fn address_v4(octets: [u8; 4]) -> String {
    std::net::Ipv4Addr::from(octets).to_string()
}

/// IPv6 地址：网络字节序 16 字节 → RFC 5952 规范压缩字符串。
#[must_use]
pub(crate) fn address_v6(octets: [u8; 16]) -> String {
    std::net::Ipv6Addr::from(octets).to_string()
}
