//! C1 macOS lsof 输出解析测试（在 Linux 执行）。
//!
//! 原因：macOS 的 cfg 代码（OS 采集与 FFI）无法在本机编译，故仅通过
//! `#[path]` 引入 src/macos/ 下与 OS 无关的纯解析模块（只依赖 std +
//! runquiry-core 类型）在 Linux 直接编译运行；OS 相关模块的编译验证由
//! macOS 交叉编译检查承担。
//!
//! 覆盖场景（C1 必测）：空格路径、中文路径、换行路径、字段缺失、非零退出
//! 带部分 stdout 的抢救语义。全部数据为合成值（fxt- 前缀、fixture-user）。
// lsof 的 plist / 网络归集辅助仅被 cfg(macos) 生产模块消费，本目标不含。
#![allow(missing_docs, dead_code)]

#[path = "../src/macos/lsof.rs"]
mod lsof;

use lsof::{parse_cwd_txt, parse_file_rows, parse_holder_pids, parse_open_ports};
use runquiry_core::{LockMode, Protocol};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn open_ports_parses_witr_column_format_and_salvages() -> TestResult {
    // 列格式：COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME [STATE]；
    // 首行表头跳过；UDP 无状态列按 witr 记 OPEN。
    let stdout = "COMMAND   PID  USER   FD   TYPE DEVICE SIZE/OFF NODE NAME\n\
                  fxt-app   5150 fixture-user   12u  IPv4  0x100   0  TCP 127.0.0.1:8443 (LISTEN)\n\
                  fxt-app   5150 fixture-user   13u  IPv6  0x101   0  UDP *:5353\n\
                  fxt-app   5150 fixture-user   14u  IPv4  0x102   0  TCP *:9090\n";
    let (rows, issues) = parse_open_ports(stdout);
    assert!(issues.is_empty());
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].port, 8443);
    assert_eq!(rows[0].protocol, Protocol::Tcp);
    assert_eq!(rows[0].state, "LISTEN");
    assert_eq!(rows[0].pid, Some(5150));
    assert_eq!(rows[1].port, 5353);
    assert_eq!(rows[1].protocol, Protocol::Udp6);
    assert_eq!(rows[1].state, "OPEN");
    assert_eq!(rows[2].address, "0.0.0.0");
    assert_eq!(rows[2].protocol, Protocol::Tcp);
    Ok(())
}

#[test]
fn open_ports_unknown_owner_kept_with_pid_none() -> TestResult {
    // PID 不可解析的行：条目保留为 pid: None + 诊断（core 端口后置条件；
    // witr 静默跳过，为满足 NetworkInventory 契约而保留——已披露偏差）。
    let stdout = "COMMAND   PID  USER   FD   TYPE DEVICE SIZE/OFF NODE NAME\n\
                  fxt-app  nopriv fixture-user  12u  IPv4  0x100   0  TCP 127.0.0.1:8443 (LISTEN)\n";
    let (rows, issues) = parse_open_ports(stdout);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].pid, None);
    assert_eq!(issues.len(), 1);
    Ok(())
}

#[test]
fn open_ports_rejects_unformable_rows_with_diagnostics() -> TestResult {
    // 协议既非 TCP 也非 UDP（模型无法承载 UNKNOWN）与端口 0：跳过 + 诊断。
    let stdout = "COMMAND   PID  USER   FD   TYPE DEVICE SIZE/OFF NODE NAME\n\
                  fxt-app   5150 fixture-user   12u  KQUEUE  0x100   0  KQUEUE 1.2.3.4:123\n\
                  fxt-app   5150 fixture-user   14u  IPv4  0x102   0  TCP 127.0.0.1:0\n";
    let (rows, issues) = parse_open_ports(stdout);
    assert!(rows.is_empty());
    assert_eq!(issues.len(), 2);
    Ok(())
}

#[test]
fn file_rows_recover_paths_with_spaces_and_cjk() -> TestResult {
    // NAME 列字段重拼接：空格与中文路径可恢复（witr strings.Join 同语义）。
    let stdout = "COMMAND   PID  USER   FD   TYPE DEVICE SIZE/OFF NODE NAME\n\
                  fxt-app   5150 fixture-user  cwd    DIR   1,4        1024   2 /Users/fixture-user/Library/RunquiryFixtures/my dir\n\
                  fxt-daemon 5150 fixture-user   5uW  REG   1,4        4096    3 /Users/fixture-user/Library/RunquiryFixtures/锁 文件.lock\n";
    let (rows, issues) = parse_file_rows(stdout);
    assert!(issues.is_empty());
    assert_eq!(
        rows[0].path,
        "/Users/fixture-user/Library/RunquiryFixtures/my dir"
    );
    assert_eq!(rows[0].fd, None); // cwd 非数字 FD。
    assert_eq!(rows[0].lock_mode, None);
    assert_eq!(
        rows[1].path,
        "/Users/fixture-user/Library/RunquiryFixtures/锁 文件.lock"
    );
    assert_eq!(rows[1].fd, Some(5));
    assert_eq!(rows[1].lock_mode, Some(LockMode::Write));
    Ok(())
}

#[test]
fn file_rows_newline_paths_are_split_by_lines() -> TestResult {
    // 换行路径：lsof 未使用 -0/转义策略，按行切分导致路径不可靠（witr 同
    // 语义；如实保留行为并在代码注释与交付报告披露，不做平台外修补）。
    let stdout = "COMMAND   PID  USER   FD   TYPE DEVICE SIZE/OFF NODE NAME\n\
                  fxt-app   5150 fixture-user   3r   REG   1,4        4096    4 /Users/fixture-user/Library/RunquiryFixtures/broken\n\
                  5150 fixture-user   3r   REG   1,4        4096    4 path\n";
    let (rows, issues) = parse_file_rows(stdout);
    // 第二段「行」字段不足 9 列：记诊断并跳过。
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].path,
        "/Users/fixture-user/Library/RunquiryFixtures/broken"
    );
    assert_eq!(issues.len(), 1);
    Ok(())
}

#[test]
fn file_rows_missing_fields_and_bad_pid_are_reported() -> TestResult {
    let stdout = "COMMAND   PID  USER   FD   TYPE DEVICE SIZE/OFF NODE NAME\n\
                  fxt-app   5150 fixture-user   3u\n\
                  fxt-app   nopriv fixture-user   4u   REG   1,4    4096  5 /Users/fixture-user/Library/RunquiryFixtures/a\n";
    let (rows, issues) = parse_file_rows(stdout);
    assert!(rows.is_empty());
    assert_eq!(issues.len(), 2);
    Ok(())
}

#[test]
fn file_rows_lock_flags_map_to_read_write_modes() -> TestResult {
    let stdout = "COMMAND   PID  USER   FD   TYPE DEVICE SIZE/OFF NODE NAME\n\
                  fxt-lock  5150 fixture-user   5uW  REG   1,4    1  6 /Users/fixture-user/Library/RunquiryFixtures/fxt-write.lock\n\
                  fxt-lock  5151 fixture-user   6rR  REG   1,4    1  7 /Users/fixture-user/Library/RunquiryFixtures/fxt-read.lock\n\
                  fxt-lock  5152 fixture-user   7uU  REG   1,4    1  8 /Users/fixture-user/Library/RunquiryFixtures/fxt-rw.lock\n";
    let (rows, _) = parse_file_rows(stdout);
    let modes: Vec<Option<LockMode>> = rows.iter().map(|row| row.lock_mode).collect();
    assert_eq!(
        modes,
        vec![
            Some(LockMode::Write),
            Some(LockMode::Read),
            Some(LockMode::ReadWrite),
        ]
    );
    Ok(())
}

#[test]
fn cwd_txt_salvage_semantics_match_witr() -> TestResult {
    // lsof -F fn：f 行定义当前 FD，n 行为路径；cwd 与 txt 分别归位。
    let stdout = "p5150\nfcwd\nn/Users/fixture-user/Library/RunquiryFixtures/c w d\nftxt\nn/Users/fixture-user/Library/RunquiryFixtures/bin/fxt-app\n";
    let (cwd, txt) = parse_cwd_txt(stdout);
    assert_eq!(
        cwd,
        Some("/Users/fixture-user/Library/RunquiryFixtures/c w d".into())
    );
    assert_eq!(
        txt,
        Some("/Users/fixture-user/Library/RunquiryFixtures/bin/fxt-app".into())
    );
    // 空输出（进程无 cwd 可读）：均为 None，不报错。
    assert_eq!(parse_cwd_txt(""), (None, None));
    Ok(())
}

#[test]
fn holder_pids_parse_p_prefixed_lines() -> TestResult {
    assert_eq!(
        parse_holder_pids("p5150\np5151\nq5152\np0\n"),
        vec![5150, 5151]
    );
    Ok(())
}

#[test]
fn open_ports_without_header_still_parses() -> TestResult {
    let (rows, issues) = parse_open_ports(
        "fxt-app   5150 fixture-user   12u  IPv4  0x100   0  TCP 127.0.0.1:8443 (LISTEN)\n",
    );
    assert!(issues.is_empty());
    assert_eq!(rows.len(), 1);
    Ok(())
}
