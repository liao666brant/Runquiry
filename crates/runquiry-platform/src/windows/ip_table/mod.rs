//! IP Helper 表字节解码与 Socket 条目构造（纯逻辑，无 OS 依赖）。
//!
//! 本目录依赖 `std` + `runquiry-core`，可在任何平台编译；Linux 上经
//! `#[path = "../src/windows/ip_table/mod.rs"]` 由 `tests/windows_ip_table.rs`
//! 直接执行。行偏移与 `windows-sys 0.61.2` 的 `MIB_*_OWNER_PID` 结构 ABI
//! 一致（x64 与 x86 相同：全部字段为 u32 / [u8; N]，无指针无填充差异）；
//! 端口与地址均为网络字节序存储，状态名按 witr `netstat` 风格映射
//! （`LISTENING` → `LISTEN`；UDP 无状态，记 `OPEN`）。
//!
//! 结构：[`wire`](self::wire)（表结构、行布局偏移与字段解码）、
//! [`parse`](self::parse)（整表行解析与条目构造）。条目构造统一复用 core
//! 的 `validate_socket_entry`（TCP/UDP 必有合法端口 1..=65535），端口 0 /
//! 越界的行由调用方跳过并记解析诊断，不得编造。

mod parse;
mod wire;

pub(super) use parse::{IpTableError, parse_table, to_socket_entry};
pub(super) use wire::{SocketRow, TableKind, decode_port};
// 仅内部与 `#[path]` 纯解析测试使用的 wire 项：cfg(test) 门控，避免
// 非 test 构建报未使用导入。
#[cfg(test)]
pub(super) use wire::{address_v4, address_v6, tcp_state_name};

#[cfg(test)]
mod tests {
    use super::{
        IpTableError, SocketRow, TableKind, address_v4, address_v6, decode_port, parse_table,
        tcp_state_name, to_socket_entry,
    };

    use runquiry_core::{Pid, Port, Protocol};

    /// 端口的网络字节序 DWORD（内存字节 = `port.to_be_bytes()`）。
    fn net_port(port: u16) -> u32 {
        u32::from(port.swap_bytes())
    }

    fn row_bytes(values: [u32; 6]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }

    /// 测试内错误传播：`IpTableError` 未实现 `std::error::Error`（不为此
    /// 改动生产公共 API），经 `Debug` 文案映射为 String 后 `?` 传播。
    fn ok<T, E: std::fmt::Debug>(context: &str, result: Result<T, E>) -> Result<T, String> {
        result.map_err(|error| format!("{context}: {error:?}"))
    }

    fn table(kind: TableKind, rows: &[Vec<u8>]) -> Result<Vec<u8>, String> {
        let count = u32::try_from(rows.len()).map_err(|_| String::from("合成行数超出 u32"))?;
        let mut buf = count.to_le_bytes().to_vec();
        for row in rows {
            buf.extend_from_slice(row);
        }
        assert_eq!(buf.len(), 4 + rows.len() * kind.row_bytes());
        Ok(buf)
    }

    #[test]
    fn port_decodes_network_byte_order_and_rejects_zero() {
        assert_eq!(decode_port(net_port(443)), Port::new(443).ok());
        assert_eq!(decode_port(net_port(8080)), Port::new(8080).ok());
        assert_eq!(decode_port(0), None);
        assert_eq!(decode_port(0xABCD_0000), None);
    }

    #[test]
    fn addresses_format_canonically() {
        assert_eq!(address_v4([127, 0, 0, 1]), "127.0.0.1");
        assert_eq!(
            address_v6([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]),
            "::1"
        );
        assert_eq!(
            address_v6([0x20, 0x01, 0x0D, 0xB8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]),
            "2001:db8::1"
        );
    }

    #[test]
    fn tcp_states_map_to_netstat_style_names() {
        assert_eq!(tcp_state_name(2), "LISTEN");
        assert_eq!(tcp_state_name(5), "ESTABLISHED");
        assert_eq!(tcp_state_name(11), "TIME_WAIT");
        assert_eq!(tcp_state_name(999), "UNKNOWN");
    }

    #[test]
    fn tcp_v4_listen_row_parses_with_owner_pid() -> Result<(), Box<dyn std::error::Error>> {
        // state=LISTEN, 0.0.0.0:8443, 0.0.0.0:0, pid=6260（fixture 合成值）。
        let row = row_bytes([2, 0, net_port(8443), 0, 0, 6260]);
        let buf = table(TableKind::TcpV4, &[row])?;
        let rows = ok("TcpV4 表", parse_table(TableKind::TcpV4, &buf))?;
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.address, "0.0.0.0");
        assert_eq!(row.state, "LISTEN");
        assert_eq!(decode_port(row.port_raw), Port::new(8443).ok());
        assert_eq!(row.owner_pid_raw, 6260);
        // 远端端口 0（监听行）在条目构造时归一为 None。
        assert_eq!(
            ok("条目构造", to_socket_entry(row, Pid::new(6260).ok()))?
                .ok_or("监听行应构造出条目")?
                .remote_addr,
            None
        );

        let entry = ok("条目构造", to_socket_entry(row, Pid::new(6260).ok()))?
            .ok_or("监听行应构造出条目")?;
        assert_eq!(entry.protocol, Protocol::Tcp);
        assert_eq!(entry.port, Port::new(8443).ok());
        assert_eq!(entry.owner_pid, Pid::new(6260).ok());
        Ok(())
    }

    #[test]
    fn tcp_v6_established_row_formats_remote() -> Result<(), Box<dyn std::error::Error>> {
        let mut row = vec![0u8; 56];
        // 本地 [::1]:9000。
        row[0..16].copy_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        row[16..20].copy_from_slice(&7u32.to_le_bytes()); // scope id
        row[20..24].copy_from_slice(&net_port(9000).to_le_bytes());
        // 对端 2001:db8::1:4242。
        row[24..40].copy_from_slice(&[0x20, 0x01, 0x0D, 0xB8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        row[44..48].copy_from_slice(&net_port(4242).to_le_bytes());
        row[48..52].copy_from_slice(&5u32.to_le_bytes()); // ESTAB
        row[52..56].copy_from_slice(&4242u32.to_le_bytes());
        let buf = table(TableKind::TcpV6, &[row])?;
        let rows = ok("TcpV6 表", parse_table(TableKind::TcpV6, &buf))?;
        let row = &rows[0];
        assert_eq!(row.address, "::1");
        assert_eq!(row.state, "ESTABLISHED");
        assert_eq!(
            row.remote.as_ref().map(|(addr, _)| addr.as_str()),
            Some("2001:db8::1")
        );
        let entry = ok("条目构造", to_socket_entry(row, None))?.ok_or("行被意外丢弃")?;
        assert_eq!(entry.remote_addr, Some(String::from("2001:db8::1:4242")));
        Ok(())
    }

    #[test]
    fn udp_row_uses_open_state_and_owner_pid() -> Result<(), Box<dyn std::error::Error>> {
        let mut row = vec![0u8; 12];
        row[..4].copy_from_slice(&[0, 0, 0, 0]);
        row[4..8].copy_from_slice(&net_port(5353).to_le_bytes());
        row[8..12].copy_from_slice(&123u32.to_le_bytes());
        let buf = table(TableKind::UdpV4, &[row])?;
        let rows = ok("UdpV4 表", parse_table(TableKind::UdpV4, &buf))?;
        let row = &rows[0];
        assert_eq!(row.state, "OPEN");
        assert_eq!(row.remote, None);
        let entry =
            ok("条目构造", to_socket_entry(row, Pid::new(123).ok()))?.ok_or("行被意外丢弃")?;
        assert_eq!(entry.owner_pid, Pid::new(123).ok());
        Ok(())
    }

    #[test]
    fn unowned_row_maps_to_none_owner_not_dropped() -> Result<(), Box<dyn std::error::Error>> {
        let row = row_bytes([2, 0, net_port(443), 0, 0, 0]);
        let buf = table(TableKind::TcpV4, &[row])?;
        let rows = ok("TcpV4 表", parse_table(TableKind::TcpV4, &buf))?;
        let entry = ok("条目构造", to_socket_entry(&rows[0], None))?.ok_or("行被意外丢弃")?;
        assert_eq!(entry.owner_pid, None, "无属主条目保留 pid=None");
        Ok(())
    }

    #[test]
    fn port_zero_row_is_rejected_not_fabricated() -> Result<(), Box<dyn std::error::Error>> {
        let row = row_bytes([2, 0, 0, 0, 0, 123]);
        let buf = table(TableKind::TcpV4, &[row])?;
        let rows = ok("TcpV4 表", parse_table(TableKind::TcpV4, &buf))?;
        let error = ok("条目构造", to_socket_entry(&rows[0], None))
            .err()
            .ok_or("端口 0 行应被拒绝而非构造条目")?;
        assert!(error.contains("非法"));
        Ok(())
    }

    #[test]
    fn row_count_mismatch_is_typed_failure() {
        let header = 5u32.to_le_bytes();
        let mut buf = header.to_vec();
        buf.extend_from_slice(&row_bytes([2, 0, net_port(443), 0, 0, 1]));
        assert_eq!(
            parse_table(TableKind::TcpV4, &buf),
            Err(IpTableError::RowCountMismatch {
                count: 5,
                needed: 4 + 5 * 24,
                actual: 28,
            })
        );
    }

    #[test]
    fn oversized_count_is_typed_failure() {
        let buf = u32::MAX.to_le_bytes().to_vec();
        assert!(matches!(
            parse_table(TableKind::TcpV4, &buf),
            Err(IpTableError::RowCountExceedsCap { .. })
        ));
    }

    #[test]
    fn header_too_short_is_typed_failure() {
        assert_eq!(
            parse_table(TableKind::UdpV4, &[0, 0]),
            Err(IpTableError::HeaderTooShort)
        );
    }

    #[test]
    fn zero_entry_table_parses_to_empty() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            ok("空表", parse_table(TableKind::UdpV4, &[0, 0, 0, 0]))?,
            Vec::<SocketRow>::new()
        );
        Ok(())
    }

    #[test]
    fn udp_v6_row_offsets_match_abi() -> Result<(), Box<dyn std::error::Error>> {
        let mut row = vec![0u8; 28];
        row[..16].copy_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        row[16..20].copy_from_slice(&7u32.to_le_bytes()); // scope id
        row[20..24].copy_from_slice(&net_port(9443).to_le_bytes());
        row[24..28].copy_from_slice(&77u32.to_le_bytes());
        let buf = table(TableKind::UdpV6, &[row])?;
        let rows = ok("UdpV6 表", parse_table(TableKind::UdpV6, &buf))?;
        let row = &rows[0];
        assert_eq!(row.address, "::1");
        assert_eq!(decode_port(row.port_raw), Port::new(9443).ok());
        assert_eq!(row.owner_pid_raw, 77);
        assert_eq!(row.state, "OPEN");
        Ok(())
    }
}
