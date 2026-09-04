//! 目标匹配纯函数与 `SocketEntry` 输入边界规则的契约测试（B1 契约门）。
//!
//! `matches_exact_token` 的 8 个用例逐条对应 witr
//! `internal/target/resolve_test.go` 的 `TestMatchesExactToken`；socket 校验
//! 对应 `tests/support/fixtures.rs` 提升到公共 API 的边界规则。

use runquiry_core::{
    Port, Protocol, SocketEntry, matches_exact_token, matches_fuzzy, validate_socket_entry,
};

mod support;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// witr `TestMatchesExactToken` 的 8 个用例语义（完整 token / 路径段匹配）。
#[test]
fn target_exact_token_follows_witr_eight_cases() {
    // 完整参数命中可执行名。
    assert!(matches_exact_token("nginx", "/usr/bin/nginx -g daemon off"));
    // 路径中间段命中（snap 模式：core24 匹配 /snap/core24/1349/bin/python）。
    assert!(matches_exact_token(
        "core24",
        "/snap/core24/1349/bin/python"
    ));
    // token 内子串不命中：foo 不匹配 /usr/local/bin/foo-bar。
    assert!(!matches_exact_token("foo", "/usr/local/bin/foo-bar"));
    // 裸参数命中。
    assert!(matches_exact_token("node", "node server.js production"));
    // 反斜杠路径段命中（Windows 风格）。
    assert!(matches_exact_token("App", r"C:\Program Files\App\bin.exe"));
    // 部分文件名不命中：python 不匹配 /usr/bin/python3。
    assert!(!matches_exact_token("python", "/usr/bin/python3"));
    // 带扩展名的文件基名命中。
    assert!(matches_exact_token(
        "witr.exe",
        r"C:\Program Files\App\witr.exe"
    ));
    // 空命令行永不命中。
    assert!(!matches_exact_token("nginx", ""));
}

/// 函数本身大小写敏感；witr 的 exact 比较由调用方先把两侧统一转小写
/// （`name_*.go` 均先 `ToLower` 再传入 matchesExactToken）。
#[test]
fn target_exact_token_is_case_sensitive_and_callers_lowercase() {
    assert!(!matches_exact_token("Nginx", "/usr/bin/nginx"));
    assert!(matches_exact_token(
        "nginx",
        "/usr/bin/NGINX".to_lowercase().as_str()
    ));
}

/// fuzzy 匹配：大小写不敏感子串（parity：exact=false 的名称匹配）。
#[test]
fn target_fuzzy_match_is_case_insensitive_substring() {
    assert!(matches_fuzzy("NGINX", "/usr/sbin/nginx: master process"));
    assert!(matches_fuzzy("nginx", "NGINX WORKER"));
    assert!(matches_fuzzy("Foo", "barfoobar"));
    assert!(!matches_fuzzy("nginx", "postgres"));
}

fn tcp_entry(port: Option<u16>) -> SocketEntry {
    SocketEntry {
        inode: None,
        port: port.and_then(|p| Port::new(p).ok()),
        address: String::from("0.0.0.0"),
        remote_addr: None,
        state: String::from("LISTEN"),
        protocol: Protocol::Tcp,
        owner_pid: None,
    }
}

#[test]
fn target_socket_validation_accepts_protocol_port_pairs() {
    assert!(validate_socket_entry(&tcp_entry(Some(8443))).is_ok());
    let unix_ok = SocketEntry {
        inode: None,
        port: None,
        address: String::from("/opt/runquiry-fixtures/var/fxt.sock"),
        remote_addr: None,
        state: String::from("LISTEN"),
        protocol: Protocol::Unix,
        owner_pid: None,
    };
    assert!(validate_socket_entry(&unix_ok).is_ok());
}

#[test]
fn target_socket_validation_rejects_protocol_port_violations() -> TestResult {
    // Unix socket 不得携带端口。
    let unix_with_port = SocketEntry {
        port: Port::new(8443).ok(),
        protocol: Protocol::Unix,
        address: String::from("/opt/runquiry-fixtures/var/fxt.sock"),
        inode: None,
        remote_addr: None,
        state: String::from("LISTEN"),
        owner_pid: None,
    };
    let err = validate_socket_entry(&unix_with_port)
        .err()
        .ok_or_else(|| String::from("Unix socket 携带端口必须被拒绝"))?;
    assert!(
        err.contains("Unix socket 条目"),
        "错误须沿用 fixture 边界规则文案：{err}"
    );

    // TCP 条目缺少端口必须被拒绝。
    let tcp_without_port = tcp_entry(None);
    assert!(
        validate_socket_entry(&tcp_without_port).is_err(),
        "TCP 条目端口必须为 Some"
    );
    Ok(())
}

#[test]
fn target_socket_validation_covers_every_protocol() {
    let entry = |protocol: Protocol| SocketEntry {
        inode: None,
        port: Some(Port::new(80).unwrap_or(Port::MIN)),
        address: String::from("0.0.0.0"),
        remote_addr: None,
        state: String::from("LISTEN"),
        protocol,
        owner_pid: None,
    };
    for protocol in [Protocol::Tcp, Protocol::Tcp6, Protocol::Udp, Protocol::Udp6] {
        assert!(validate_socket_entry(&entry(protocol)).is_ok());
    }
    assert!(validate_socket_entry(&entry(Protocol::Unix)).is_err());
}

// ---------------------------------------------------------------------------
// B1 完整实现：五类目标的边界解析与名称/端口/容器/文件解析（parity §2）
// ---------------------------------------------------------------------------

use std::path::PathBuf;

use runquiry_core::{
    ContainerKey, ContainerMatchInput, ContainerSummary, FileLockEntry, LockMode, LockType,
    OpenPortEntry, Pid, Resolution, merge_service_pid, parse_file_path, parse_pid, parse_port,
    parse_query, resolve_containers, resolve_file_holders, resolve_name, resolve_port_owner,
    resolve_port_owner_in_sockets, scan_name_candidates,
};

use crate::support::collectors::summary;

/// 合成 PID 构造（测试值恒合法）。
fn pid(value: u32) -> Pid {
    Pid::new(value).unwrap_or(Pid::MIN)
}

/// 闭包内使用的 PID 构造（测试值恒合法）。
fn pid_value(value: u32) -> Pid {
    pid(value)
}

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

fn open_port(pid: Option<u32>, port: u16) -> OpenPortEntry {
    OpenPortEntry {
        pid: pid.and_then(|p| Pid::new(p).ok()),
        port: Port::new(port).unwrap_or(Port::MIN),
        address: String::from("0.0.0.0"),
        protocol: Protocol::Tcp,
        state: String::from("LISTEN"),
    }
}

/// 端口解析：唯一属主 / 多属主 / 属主不可知 / 无条目（parity §2 + 哨兵）。
#[test]
fn target_port_resolution_maps_entries_to_owner_resolution() -> TestResult {
    let port = Port::new(8443)?;
    // 唯一属主。
    let entries = vec![open_port(Some(7), 8443), open_port(Some(7), 9000)];
    assert_eq!(
        resolve_port_owner(&entries, port)?,
        Resolution::Unique(Pid::new(7)?)
    );
    // 多属主 → Ambiguous，完整且升序。
    let entries = vec![open_port(Some(9), 8443), open_port(Some(3), 8443)];
    let Resolution::Ambiguous(owners) = resolve_port_owner(&entries, port)? else {
        return Err(String::from("多属主应返回 Ambiguous").into());
    };
    assert_eq!(owners, vec![Pid::new(3)?, Pid::new(9)?]);
    // 有条目但属主不可知 → SocketOwnerUnknown（parity 哨兵）。
    let entries = vec![open_port(None, 8443)];
    let err = resolve_port_owner(&entries, port)
        .err()
        .ok_or_else(|| String::from("无主端口应报 socket_owner_unknown"))?;
    assert_eq!(err.code(), "socket_owner_unknown");
    // 无条目 → NotFound。
    let entries = vec![open_port(Some(7), 9000)];
    let err = resolve_port_owner(&entries, port)
        .err()
        .ok_or_else(|| String::from("无条目应报 not_found"))?;
    assert_eq!(err.code(), "not_found");
    Ok(())
}

/// `SocketEntry` 上的端口解析与 `OpenPortEntry` 行为一致。
#[test]
fn target_port_resolution_on_socket_entries_matches_open_port_semantics() -> TestResult {
    let socket = |owner: Option<u32>, port: u16| SocketEntry {
        inode: None,
        port: Port::new(port).ok(),
        address: String::from("0.0.0.0"),
        remote_addr: None,
        state: String::from("LISTEN"),
        protocol: Protocol::Tcp,
        owner_pid: owner.and_then(|p| Pid::new(p).ok()),
    };
    let port = Port::new(80)?;
    let sockets = vec![socket(Some(2), 80), socket(Some(2), 443)];
    assert_eq!(
        resolve_port_owner_in_sockets(&sockets, port)?,
        Resolution::Unique(Pid::new(2)?)
    );
    let sockets = vec![socket(None, 80)];
    assert_eq!(
        resolve_port_owner_in_sockets(&sockets, port)
            .err()
            .map(|e| e.code().to_string()),
        Some(String::from("socket_owner_unknown"))
    );
    Ok(())
}

fn container(name: Option<&str>, runtime: &str, id: &str, image: Option<&str>) -> ContainerSummary {
    ContainerSummary {
        key: ContainerKey {
            runtime: String::from(runtime),
            id: String::from(id),
        },
        name: name.map(String::from),
        image: image.map(String::from),
        status: None,
        health: None,
        host_pid: None,
        started_at: None,
    }
}

/// 容器解析：五字段匹配（name/image/command/compose project/service）、
/// exact 全等 / fuzzy 子串、空字段跳过、大小写在 API 内部统一。
#[test]
fn target_container_resolution_matches_five_fields_with_exact_and_fuzzy() -> TestResult {
    let summary = container(Some("fxt-web"), "docker", "abc123", Some("nginx:latest"));
    let key = summary.key.clone();
    let inputs = vec![ContainerMatchInput {
        summary: &summary,
        command: Some("nginx -g daemon off"),
        compose_project: Some("fxt-stack"),
        compose_service: Some("web"),
    }];
    // fuzzy：name / compose service 子串（大小写不敏感）。
    assert_eq!(
        resolve_containers(&inputs, "fxt-web", false)?,
        Resolution::Unique(key.clone())
    );
    assert_eq!(
        resolve_containers(&inputs, "WEB", false)?,
        Resolution::Unique(key.clone())
    );
    // exact：字段全等命中（大小写不敏感）。
    assert_eq!(
        resolve_containers(&inputs, "NGINX:LATEST", true)?,
        Resolution::Unique(key)
    );
    // exact：非完整字段不命中（"fxt" ≠ "fxt-web"）→ NotFound。
    let err = resolve_containers(&inputs, "fxt", true)
        .err()
        .ok_or_else(|| String::from("exact 部分串不得命中"))?;
    assert_eq!(err.code(), "not_found");
    // 无命中 → NotFound。
    let err = resolve_containers(&inputs, "postgres", false)
        .err()
        .ok_or_else(|| String::from("无命中应为 not_found"))?;
    assert_eq!(err.code(), "not_found");
    Ok(())
}

/// 容器解析：按 runtime+id 去重（不得按短 ID 合并）、稳定排序、多结果完整候选。
#[test]
fn target_container_resolution_dedups_by_runtime_and_id_and_sorts() -> TestResult {
    let a = container(Some("fxt-a"), "docker", "aaa111", None);
    let b = container(Some("fxt-b"), "docker", "aaa111999999", None);
    let c = container(Some("fxt-c"), "podman", "bbb222", None);
    let mut inputs = Vec::new();
    for s in [&a, &a, &b, &c] {
        inputs.push(ContainerMatchInput {
            summary: s,
            command: None,
            compose_project: None,
            compose_service: None,
        });
    }
    let resolved = resolve_containers(&inputs, "fxt-", false)?;
    let Resolution::Ambiguous(keys) = &resolved else {
        return Err(String::from("多容器命中应返回 Ambiguous").into());
    };
    // docker|aaa111 与 docker|aaa111999999 是不同键（不得按短 ID 合并），加 podman|bbb222。
    assert_eq!(keys.len(), 3, "runtime|id 去重后三个键全部保留");
    assert_eq!(
        keys[0],
        ContainerKey {
            runtime: String::from("docker"),
            id: String::from("aaa111")
        }
    );
    assert_eq!(
        keys[2],
        ContainerKey {
            runtime: String::from("podman"),
            id: String::from("bbb222")
        }
    );
    Ok(())
}

/// 文件解析：委托 holders 结果；空 → NotFound（subject 含路径）；命中 → Unique/Ambiguous。
#[test]
fn target_file_resolution_delegates_to_holders_results() -> TestResult {
    let path = PathBuf::from("/opt/runquiry-fixtures/var/fxt.log");
    let entry = |pid: u32| FileLockEntry {
        pid: pid_value(pid),
        process: String::from("fxt-daemon"),
        path: path.clone(),
        lock_type: LockType::Flock,
        mode: LockMode::Write,
    };
    let err = resolve_file_holders(&[], &path)
        .err()
        .ok_or_else(|| String::from("空 holders 应报 not_found"))?;
    assert_eq!(err.code(), "not_found");
    assert!(
        err.to_string().contains("fxt.log"),
        "NotFound subject 须含路径：{err}"
    );

    assert_eq!(
        resolve_file_holders(&[entry(7)], &path)?,
        Resolution::Unique(Pid::new(7)?)
    );
    let Resolution::Ambiguous(pids) = resolve_file_holders(&[entry(9), entry(3)], &path)? else {
        return Err(String::from("多持有者应返回 Ambiguous").into());
    };
    assert_eq!(pids, vec![Pid::new(3)?, Pid::new(9)?], "去重升序");
    Ok(())
}
