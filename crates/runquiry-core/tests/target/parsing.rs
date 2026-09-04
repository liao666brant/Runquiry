//! 五类调查目标的输入解析契约。

use std::path::PathBuf;

use runquiry_core::{parse_file_path, parse_pid, parse_port, parse_query};

use super::TestResult;

/// PID 边界解析：仅接受正整数字符串（parity：strconv.Atoi + pid<=0 拒绝）。
#[test]
fn target_parse_pid_accepts_only_positive_integers() -> TestResult {
    assert_eq!(parse_pid("42")?.get(), 42);
    assert_eq!(parse_pid(" 7 ")?.get(), 7);
    for bad in ["0", "-1", "abc", "", "3.5", "999999999999999999999"] {
        let err = parse_pid(bad)
            .err()
            .ok_or_else(|| format!("{bad:?} 应被拒绝"))?;
        assert_eq!(err.code(), "invalid_target", "{bad:?} 应为 invalid_target");
    }
    Ok(())
}

/// 端口边界解析：仅接受 1-65535（parity：invalid port must be between 1 and 65535）。
#[test]
fn target_parse_port_accepts_only_valid_range() -> TestResult {
    assert_eq!(parse_port("1")?.get(), 1);
    assert_eq!(parse_port("65535")?.get(), 65535);
    for bad in ["0", "65536", "abc", "", "-1"] {
        let err = parse_port(bad)
            .err()
            .ok_or_else(|| format!("{bad:?} 应被拒绝"))?;
        assert_eq!(err.code(), "invalid_target");
    }
    Ok(())
}

/// 文件路径边界解析：非空即原样保留（归一化由平台 `FileInventory` 承担）。
#[test]
fn target_parse_file_path_keeps_input_verbatim() -> TestResult {
    assert_eq!(
        parse_file_path("/opt/runquiry-fixtures/var/fxt.log")?,
        PathBuf::from("/opt/runquiry-fixtures/var/fxt.log")
    );
    // 不做归一化：相对段原样保留，语义归 FileInventory。
    assert_eq!(parse_file_path("a/../b")?, PathBuf::from("a/../b"));
    let err = parse_file_path("  ")
        .err()
        .ok_or_else(|| String::from("空路径应被拒绝"))?;
    assert_eq!(err.code(), "invalid_target");
    Ok(())
}

/// 名称/容器查询串边界解析：trim 后非空。
#[test]
fn target_parse_query_rejects_blank_and_trims() -> TestResult {
    assert_eq!(parse_query("  nginx  ")?.as_str(), "nginx");
    let err = parse_query("   ")
        .err()
        .ok_or_else(|| String::from("空查询应被拒绝"))?;
    assert_eq!(err.code(), "invalid_target");
    Ok(())
}
