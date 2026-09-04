//! 按名称解析进程的契约。

use runquiry_core::{Pid, Resolution, merge_service_pid, resolve_name, scan_name_candidates};

use crate::support::collectors::summary;

use super::TestResult;

/// 合成 PID 构造（测试值恒合法）。
fn pid(value: u32) -> Pid {
    Pid::new(value).unwrap_or(Pid::MIN)
}

fn fixture_inventory() -> Vec<runquiry_core::ProcessSummary> {
    vec![
        summary(12, Some(1), "nginx-worker", Some("/usr/sbin/nginx-worker")),
        summary(7, Some(1), "nginx", Some("nginx -g daemon off;")),
        summary(
            9,
            Some(7),
            "python3",
            Some("/snap/core24/1349/bin/python -c print(1)"),
        ),
        summary(11, Some(9), "foo-bar", Some("/usr/local/bin/foo-bar")),
        summary(4242, Some(1), "4242", Some("4242 --serve")),
    ]
}

/// fuzzy 名称匹配：大小写不敏感子串，先 comm 后完整命令行（parity §2）。
#[test]
fn target_name_scan_fuzzy_matches_comm_then_cmdline_case_insensitively() {
    let inv = fixture_inventory();
    let pids = scan_name_candidates(&inv, "NGINX", false, &[]);
    assert_eq!(
        pids,
        vec![pid(7), pid(12)],
        "comm 子串命中 7/12，大小写不敏感"
    );
    // cmdline 命中（comm 未命中时回退完整命令行子串）。
    let pids = scan_name_candidates(&inv, "daemon off", false, &[]);
    assert_eq!(pids, vec![pid(7)]);
    let pids = scan_name_candidates(&inv, "core24", false, &[]);
    assert_eq!(pids, vec![pid(9)]);
}

/// exact 名称匹配：comm 全等，或完整命令行做完整 token / 路径段匹配
/// （两侧小写化在 API 内部完成）。
#[test]
fn target_name_scan_exact_requires_full_token_or_comm_equality() {
    let inv = fixture_inventory();
    // comm 全等命中，comm 子串不命中。
    let pids = scan_name_candidates(&inv, "nginx", true, &[]);
    assert_eq!(pids, vec![pid(7)]);
    // 路径段完整 token 命中（snap 模式）。
    let pids = scan_name_candidates(&inv, "core24", true, &[]);
    assert_eq!(pids, vec![pid(9)]);
    // token 内子串不命中（foo 不匹配 /usr/local/bin/foo-bar）。
    let pids = scan_name_candidates(&inv, "foo", true, &[]);
    assert!(pids.is_empty());
    // 大写查询在 API 内部统一小写化。
    let pids = scan_name_candidates(&inv, "NGINX", true, &[]);
    assert_eq!(pids, vec![pid(7)]);
}

/// 纯数字查询守卫：查询串等于某 PID 十进制串时不得按名称命中该进程
/// （parity：lowerName == strconv.Itoa(pid) 时 continue；守卫只作用于该 PID 自身）。
#[test]
fn target_name_scan_skips_numeric_query_equal_to_pid() {
    let inv = fixture_inventory();
    let pids = scan_name_candidates(&inv, "4242", true, &[]);
    assert!(pids.is_empty(), "查询 4242 不得命中 PID 4242 的进程");
    let other = vec![
        summary(5, Some(1), "4242", None),
        summary(4242, Some(1), "4242", None),
    ];
    let pids = scan_name_candidates(&other, "4242", true, &[]);
    assert_eq!(pids, vec![pid(5)]);
}

/// ignored 链排除：自身及祖先链中的 PID 不作为候选返回。
#[test]
fn target_name_scan_excludes_ignored_pids() -> TestResult {
    let inv = fixture_inventory();
    let pids = scan_name_candidates(&inv, "nginx", false, &[Pid::new(7)?, Pid::new(12)?]);
    assert!(pids.is_empty(), "ignored 链内的命中必须被排除");
    Ok(())
}

/// 候选去重与升序；多结果 Ambiguous 携带完整稳定排序候选，唯一结果 Unique。
#[test]
fn target_name_resolution_dedup_sorts_and_reports_ambiguity() -> TestResult {
    let inv = vec![
        summary(30, Some(1), "nginx", None),
        summary(3, Some(1), "nginx", None),
        summary(3, Some(1), "nginx", None),
    ];
    let resolved = resolve_name(&inv, "nginx", true, &[], None)?;
    let Resolution::Ambiguous(candidates) = &resolved else {
        return Err(String::from("多个命中应返回 Ambiguous").into());
    };
    assert_eq!(
        candidates,
        &vec![Pid::new(3)?, Pid::new(30)?],
        "去重且按 PID 升序"
    );

    let single = vec![summary(7, Some(1), "nginx", None)];
    assert_eq!(
        resolve_name(&single, "nginx", true, &[], None)?,
        Resolution::Unique(Pid::new(7)?)
    );
    Ok(())
}

/// 无结果 → NotFound；systemd 服务 PID 仅在扫描零命中时参与（parity：
/// 只在 /proc 扫描零命中时才回退 systemctl）。
#[test]
fn target_name_resolution_reports_not_found_and_merges_service_pid() -> TestResult {
    let inv = fixture_inventory();
    let err = resolve_name(&inv, "不存在的进程", true, &[], None)
        .err()
        .ok_or_else(|| String::from("无结果应为 NotFound"))?;
    assert_eq!(err.code(), "not_found");

    // 扫描零命中 + 服务 PID → Unique（服务 PID）。
    let empty: Vec<runquiry_core::ProcessSummary> = Vec::new();
    let resolved = resolve_name(&empty, "fxt.service", true, &[], Some(Pid::new(88)?))?;
    assert_eq!(resolved, Resolution::Unique(Pid::new(88)?));

    // 扫描有命中时服务 PID 不参与。
    let resolved = resolve_name(&inv, "nginx", true, &[], Some(Pid::new(88)?))?;
    assert_eq!(resolved.count(), 1);
    assert_eq!(resolved, Resolution::Unique(pid(7)));

    // 可组合合并：服务 PID 排首位、去重、升序。
    assert_eq!(
        merge_service_pid(Some(Pid::new(3)?), &[Pid::new(3)?, Pid::new(9)?]),
        vec![Pid::new(3)?, Pid::new(9)?]
    );
    Ok(())
}
