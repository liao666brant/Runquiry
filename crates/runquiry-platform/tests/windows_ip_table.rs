//! Windows IP Helper 表解析的 Linux 编译入口（见 `windows_utf16.rs` 头注释）。

#[path = "../src/windows/ip_table/mod.rs"]
mod ip_table;

#[test]
fn ip_table_module_compiles_and_smoke_parses() {
    let buf = [0u8, 0, 0, 0];
    assert!(
        ip_table::parse_table(ip_table::TableKind::TcpV4, &buf)
            .is_ok_and(|rows| rows.is_empty()),
        "空表应成功解析为空行列表"
    );
}