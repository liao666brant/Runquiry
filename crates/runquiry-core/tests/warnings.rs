//! B1 契约门：告警相关已实现类型的契约测试——健康状态标签、进程操作错误码
//! 与告警输入字段的序列化。完整告警规则测试在契约冻结后的下一阶段补齐
//! （届时基于本文件的类型契约展开）。

use runquiry_core::{HealthStatus, InspectError, SourceType};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// witr Health 字段的五种标签（parity §1：healthy/zombie/stopped/high-cpu/high-mem）。
#[test]
fn warnings_health_status_labels_match_witr_strings() {
    assert_eq!(HealthStatus::Healthy.as_str(), "healthy");
    assert_eq!(HealthStatus::Zombie.as_str(), "zombie");
    assert_eq!(HealthStatus::Stopped.as_str(), "stopped");
    assert_eq!(HealthStatus::HighCpu.as_str(), "high-cpu");
    assert_eq!(HealthStatus::HighMem.as_str(), "high-mem");
    assert_eq!(HealthStatus::Unknown.as_str(), "unknown");
}

#[test]
fn warnings_health_status_serializes_to_witr_labels() -> TestResult {
    for (status, label) in [
        (HealthStatus::Healthy, "healthy"),
        (HealthStatus::Zombie, "zombie"),
        (HealthStatus::Stopped, "stopped"),
        (HealthStatus::HighCpu, "high-cpu"),
        (HealthStatus::HighMem, "high-mem"),
        (HealthStatus::Unknown, "unknown"),
    ] {
        let text = serde_json::to_string(&status)?;
        assert_eq!(
            text,
            format!(r#""{label}""#),
            "序列化值必须与 witr 标签一致"
        );
        let back: HealthStatus = serde_json::from_str(&text)?;
        assert_eq!(back, status);
    }
    Ok(())
}

#[test]
fn warnings_health_status_defaults_to_unknown() {
    // #[serde(default)] 的缺省健康状态是 Unknown（未采集/不可判定）。
    assert_eq!(HealthStatus::default(), HealthStatus::Unknown);
    assert_eq!(HealthStatus::default().as_str(), "unknown");
}

/// 来源类型码是告警分支（如 unknown 来源告警）的稳定判据。
#[test]
fn warnings_source_type_code_is_stable_for_warning_branches() {
    assert_eq!(SourceType::Unknown.code(), "unknown");
    assert_eq!(SourceType::Systemd.code(), "systemd");
}

/// parity 哨兵 ErrSocketOwnerUnknown：端口有 socket 但属主不可知。
#[test]
fn warnings_socket_owner_unknown_carries_stable_code() {
    let err = InspectError::SocketOwnerUnknown {
        subject: String::from("端口 8443"),
    };
    assert_eq!(err.code(), "socket_owner_unknown");
    let display = err.to_string();
    assert!(
        display.contains("端口 8443"),
        "Display 须携带主题供 UI 呈现：{display}"
    );
}

// ---------------------------------------------------------------------------
// B1 完整实现：告警规则全量对照（parity §6，顺序与 witr Warnings 逐条一致）
// ---------------------------------------------------------------------------

use std::time::{Duration, SystemTime};

use runquiry_core::{
    HealthStatus as Health, Pid, ProcessIdentity, Protocol, SocketEntry, WarningKind,
};

/// 合成链尾进程（默认无告警触发条件，由用例逐项打开）。
fn target() -> runquiry_core::ProcessSummary {
    let identity = ProcessIdentity::new(
        Pid::new(5).unwrap_or(Pid::MIN),
        Some(SystemTime::UNIX_EPOCH + Duration::from_hours(1)),
        None,
    );
    runquiry_core::ProcessSummary {
        identity,
        parent_pid: None,
        command: String::from("fxt-daemon"),
        command_line: None,
        user: Some(String::from("fixture-user")),
        health: Health::Unknown,
        container: None,
        exe_deleted: false,
        capabilities: Vec::new(),
    }
}

fn listen_socket(address: &str, state: &str) -> SocketEntry {
    SocketEntry {
        inode: None,
        port: runquiry_core::Port::new(8443).ok(),
        address: String::from(address),
        remote_addr: None,
        state: String::from(state),
        protocol: Protocol::Tcp,
        owner_pid: Some(Pid::new(5).unwrap_or(Pid::MIN)),
    }
}

fn kinds(warnings: &[runquiry_core::Warning]) -> Vec<WarningKind> {
    warnings.iter().map(runquiry_core::Warning::kind).collect()
}

/// 告警顺序与 witr `Warnings` 函数体 append 顺序逐条一致、确定性。
#[test]
fn warnings_warning_order_matches_witr_append_sequence() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_hours(91 * 24);
    let mut t = target();
    t.health = Health::Zombie;
    t.user = Some(String::from("root"));
    t.exe_deleted = true;
    t.capabilities = vec![String::from("CAP_SYS_ADMIN")]; // root 时不告警（互斥）
    t.container = Some(runquiry_core::ContainerContext::new(
        String::from("fxt-id"),
        String::from("docker"),
        Some(runquiry_core::HealthcheckStatus::Absent),
    ));
    let sockets = vec![listen_socket("0.0.0.0", "LISTEN")];
    let details = runquiry_core::ProcessDetails {
        identity: t.identity.clone(),
        cpu_percent: None,
        memory_rss_bytes: None,
        memory_percent: None,
        working_dir: Some(std::path::PathBuf::from("/tmp")),
        environment: Vec::new(),
        children: Vec::new(),
        memory: None,
        io: None,
        open_files: Vec::new(),
        fd_count: None,
        fd_limit: None,
    };
    let ctx = runquiry_core::WarningsContext {
        target: &t,
        sockets: &sockets,
        details: Some(&details),
        service: Some(String::from("totally-unrelated.service")),
        restart_count: 6,
        source_type: SourceType::Unknown,
        is_windows: false,
        now,
    };
    let warnings = runquiry_core::warnings(&ctx);
    assert_eq!(
        kinds(&warnings),
        vec![
            WarningKind::ServiceRestart,
            WarningKind::Zombie,
            WarningKind::PublicListen,
            WarningKind::RootUser,
            WarningKind::UnknownSource,
            WarningKind::LongRunning,
            WarningKind::SuspiciousWorkingDir,
            WarningKind::ContainerNoHealthcheck,
            WarningKind::ServiceNameMismatch,
            WarningKind::DeletedBinary,
        ],
        "顺序须与 witr 逐条一致"
    );
    // 文案对照（展示层可对齐 witr 英文）。
    assert_eq!(warnings[0].message(), "Service has restarted 6 times");
    assert_eq!(warnings[1].message(), "Process is a zombie (defunct)");
    assert_eq!(
        warnings[2].message(),
        "Process is listening on a public interface"
    );
    assert_eq!(warnings[3].message(), "Process is running as root");
    assert_eq!(
        warnings[4].message(),
        "No known supervisor or service manager detected"
    );
    assert_eq!(
        warnings[5].message(),
        "Process has been running for over 90 days"
    );
    assert_eq!(
        warnings[6].message(),
        "Process is running from a suspicious working directory: /tmp"
    );
    assert_eq!(
        warnings[7].message(),
        "Container has no healthcheck configured"
    );
    assert_eq!(
        warnings[8].message(),
        "Service name and process name do not match"
    );
    assert_eq!(
        warnings[9].message(),
        "Process is running from a deleted binary (potential library injection or pending update)"
    );
}

/// 高负载与健康状态告警：zombie/stopped/high-cpu/high-mem 各自独立。
fn warnings_with_health(health: Health) -> Vec<WarningKind> {
    let sockets: Vec<SocketEntry> = Vec::new();
    let t = runquiry_core::ProcessSummary { health, ..target() };
    let ctx = runquiry_core::WarningsContext {
        target: &t,
        sockets: &sockets,
        details: None,
        service: None,
        restart_count: 0,
        source_type: SourceType::Shell,
        is_windows: false,
        now: SystemTime::UNIX_EPOCH,
    };
    kinds(&runquiry_core::warnings(&ctx))
}

#[test]
fn warnings_health_state_warnings_follow_witr_labels() {
    assert_eq!(
        warnings_with_health(Health::Stopped),
        vec![WarningKind::Stopped]
    );
    assert_eq!(
        warnings_with_health(Health::HighCpu),
        vec![WarningKind::HighCpu]
    );
    assert_eq!(
        warnings_with_health(Health::HighMem),
        vec![WarningKind::HighMem]
    );
    assert!(
        warnings_with_health(Health::Healthy).is_empty()
            && warnings_with_health(Health::Unknown).is_empty(),
        "healthy 与 unknown 不告警"
    );
}

/// 公开监听：仅 LISTEN 且 0.0.0.0 / :: 命中（witr `IsPublicBind`）。
#[test]
fn warnings_public_listen_warning_requires_listening_any_address() {
    let sockets = vec![listen_socket("127.0.0.1", "LISTEN")];
    let t = target();
    let ctx = runquiry_core::WarningsContext {
        target: &t,
        sockets: &sockets,
        details: None,
        service: None,
        restart_count: 0,
        source_type: SourceType::Shell,
        is_windows: false,
        now: SystemTime::UNIX_EPOCH,
    };
    assert!(
        runquiry_core::warnings(&ctx).is_empty(),
        "loopback 监听不告警"
    );

    let sockets = vec![listen_socket("::", "LISTEN")];
    let ctx = runquiry_core::WarningsContext {
        target: &t,
        sockets: &sockets,
        details: None,
        service: None,
        restart_count: 0,
        source_type: SourceType::Shell,
        is_windows: false,
        now: SystemTime::UNIX_EPOCH,
    };
    assert_eq!(
        kinds(&runquiry_core::warnings(&ctx)),
        vec![WarningKind::PublicListen]
    );

    // 非 LISTEN 状态不告警（出站连接到公网地址不是暴露）。
    let sockets = vec![listen_socket("0.0.0.0", "ESTAB")];
    let ctx = runquiry_core::WarningsContext {
        target: &t,
        sockets: &sockets,
        details: None,
        service: None,
        restart_count: 0,
        source_type: SourceType::Shell,
        is_windows: false,
        now: SystemTime::UNIX_EPOCH,
    };
    assert!(runquiry_core::warnings(&ctx).is_empty());
}

/// root 与危险 capabilities 互斥；危险名单与 witr 逐字一致。
#[test]
fn warnings_root_and_dangerous_capabilities_are_mutually_exclusive() {
    let sockets: Vec<SocketEntry> = Vec::new();
    let mut t = target();
    t.user = Some(String::from("root"));
    t.capabilities = vec![String::from("CAP_SYS_ADMIN"), String::from("CAP_NET_RAW")];
    let ctx = runquiry_core::WarningsContext {
        target: &t,
        sockets: &sockets,
        details: None,
        service: None,
        restart_count: 0,
        source_type: SourceType::Shell,
        is_windows: false,
        now: SystemTime::UNIX_EPOCH,
    };
    let warnings = runquiry_core::warnings(&ctx);
    assert_eq!(
        kinds(&warnings),
        vec![WarningKind::RootUser],
        "root 时 capabilities 不告警"
    );

    t.user = Some(String::from("fixture-user"));
    let ctx = runquiry_core::WarningsContext {
        target: &t,
        sockets: &sockets,
        details: None,
        service: None,
        restart_count: 0,
        source_type: SourceType::Shell,
        is_windows: false,
        now: SystemTime::UNIX_EPOCH,
    };
    let warnings = runquiry_core::warnings(&ctx);
    assert_eq!(kinds(&warnings), vec![WarningKind::DangerousCapabilities]);
    assert!(
        warnings[0].message().contains("CAP_SYS_ADMIN, CAP_NET_RAW"),
        "危险 capabilities 列表按采集顺序拼接：{}",
        warnings[0].message()
    );

    // 非危险 capability 不告警。
    t.capabilities = vec![String::from("CAP_CHOWN")];
    let ctx = runquiry_core::WarningsContext {
        target: &t,
        sockets: &sockets,
        details: None,
        service: None,
        restart_count: 0,
        source_type: SourceType::Shell,
        is_windows: false,
        now: SystemTime::UNIX_EPOCH,
    };
    assert!(runquiry_core::warnings(&ctx).is_empty());
}

/// 未知来源告警：非 Windows 触发，Windows 豁免（parity：GOOS 分支）。
#[test]
fn warnings_unknown_source_warning_respects_windows_exemption() {
    let sockets: Vec<SocketEntry> = Vec::new();
    let t = target();
    let ctx = runquiry_core::WarningsContext {
        target: &t,
        sockets: &sockets,
        details: None,
        service: None,
        restart_count: 0,
        source_type: SourceType::Unknown,
        is_windows: false,
        now: SystemTime::UNIX_EPOCH,
    };
    assert_eq!(
        kinds(&runquiry_core::warnings(&ctx)),
        vec![WarningKind::UnknownSource]
    );
    let ctx = runquiry_core::WarningsContext {
        target: &t,
        sockets: &sockets,
        details: None,
        service: None,
        restart_count: 0,
        source_type: SourceType::Unknown,
        is_windows: true,
        now: SystemTime::UNIX_EPOCH,
    };
    assert!(runquiry_core::warnings(&ctx).is_empty());
}

/// 长运行告警上下文（显式生命周期绑定到目标与 socket 列表）。
fn long_running_ctx<'a>(
    t: &'a runquiry_core::ProcessSummary,
    sockets: &'a [SocketEntry],
    started: SystemTime,
    age: Duration,
) -> runquiry_core::WarningsContext<'a> {
    runquiry_core::WarningsContext {
        target: t,
        sockets,
        details: None,
        service: None,
        restart_count: 0,
        source_type: SourceType::Shell,
        is_windows: false,
        now: started + age,
    }
}

/// 长运行告警：注入时钟 >90*24h（严格大于）；`start_time` 不可得时跳过。
#[test]
fn warnings_long_running_warning_uses_injected_clock_boundary() {
    let sockets: Vec<SocketEntry> = Vec::new();
    let day = Duration::from_hours(24);
    let started = SystemTime::UNIX_EPOCH + Duration::from_hours(1);
    let t = target();
    assert!(
        !kinds(&runquiry_core::warnings(&long_running_ctx(
            &t,
            &sockets,
            started,
            89 * day
        )))
        .contains(&WarningKind::LongRunning),
        "89 天无告警"
    );
    assert!(
        !kinds(&runquiry_core::warnings(&long_running_ctx(
            &t,
            &sockets,
            started,
            90 * day
        )))
        .contains(&WarningKind::LongRunning),
        "恰好 90 天为严格大于边界，不告警"
    );
    assert_eq!(
        kinds(&runquiry_core::warnings(&long_running_ctx(
            &t,
            &sockets,
            started,
            91 * day
        ))),
        vec![WarningKind::LongRunning],
        "91 天告警"
    );

    let t = runquiry_core::ProcessSummary {
        identity: ProcessIdentity::new(Pid::new(5).unwrap_or(Pid::MIN), None, None),
        ..target()
    };
    assert!(
        runquiry_core::warnings(&long_running_ctx(&t, &sockets, started, 200 * day)).is_empty(),
        "start_time 不可得跳过"
    );
}

/// 可疑工作目录名单与 witr `suspiciousDirs` 一致（`/`、`/tmp`、`/var/tmp`）。
#[test]
fn warnings_suspicious_working_directory_list_matches_witr() {
    let sockets: Vec<SocketEntry> = Vec::new();
    let t = target();
    for (dir, want) in [
        ("/", true),
        ("/tmp", true),
        ("/var/tmp", true),
        ("/home/fixture-user", false),
    ] {
        let details = runquiry_core::ProcessDetails {
            identity: t.identity.clone(),
            cpu_percent: None,
            memory_rss_bytes: None,
            memory_percent: None,
            working_dir: Some(std::path::PathBuf::from(dir)),
            environment: Vec::new(),
            children: Vec::new(),
            memory: None,
            io: None,
            open_files: Vec::new(),
            fd_count: None,
            fd_limit: None,
        };
        let ctx = runquiry_core::WarningsContext {
            target: &t,
            sockets: &sockets,
            details: Some(&details),
            service: None,
            restart_count: 0,
            source_type: SourceType::Shell,
            is_windows: false,
            now: SystemTime::UNIX_EPOCH,
        };
        let warnings = runquiry_core::warnings(&ctx);
        assert_eq!(
            kinds(&warnings).contains(&WarningKind::SuspiciousWorkingDir),
            want,
            "{dir}"
        );
        if want {
            assert!(
                warnings[0].message().contains(dir),
                "消息含路径：{}",
                warnings[0].message()
            );
        }
    }
}

/// 服务名与进程名不匹配：模板取 `@` 前基名；含关系不告警。
#[test]
fn warnings_service_name_mismatch_follows_witr_svc_core_semantics() {
    let sockets: Vec<SocketEntry> = Vec::new();
    let t = target();
    let ctx = |service: &str| runquiry_core::WarningsContext {
        target: &t,
        sockets: &sockets,
        details: None,
        service: Some(String::from(service)),
        restart_count: 0,
        source_type: SourceType::Systemd,
        is_windows: false,
        now: SystemTime::UNIX_EPOCH,
    };
    // 名字相关：fxt-daemon.service vs fxt-daemon（双向包含）不告警。
    assert!(runquiry_core::warnings(&ctx("fxt-daemon.service")).is_empty());
    // 无关 → 告警。
    assert_eq!(
        kinds(&runquiry_core::warnings(&ctx("nginx.service"))),
        vec![WarningKind::ServiceNameMismatch]
    );
}

/// 模板单元基名取 `@` 前段（getty@tty1 → getty），二进制以模板命名的
/// （agetty 包含 getty）不读作不匹配。
#[test]
fn warnings_service_name_mismatch_uses_template_base_name() {
    let sockets: Vec<SocketEntry> = Vec::new();
    let t = runquiry_core::ProcessSummary {
        command: String::from("agetty"),
        ..target()
    };
    let ctx = runquiry_core::WarningsContext {
        target: &t,
        sockets: &sockets,
        details: None,
        service: Some(String::from("getty@tty1.service")),
        restart_count: 0,
        source_type: SourceType::Systemd,
        is_windows: false,
        now: SystemTime::UNIX_EPOCH,
    };
    assert!(runquiry_core::warnings(&ctx).is_empty());
}

/// 可疑环境变量：`LD_PRELOAD` 先于 `DYLD_*`；DYLD 命中键排序；空值跳过。
#[test]
fn warnings_suspicious_env_warnings_are_deterministic_and_ordered() {
    let sockets: Vec<SocketEntry> = Vec::new();
    let t = target();
    let details_with = |env: &[(&str, &str)]| runquiry_core::ProcessDetails {
        identity: t.identity.clone(),
        cpu_percent: None,
        memory_rss_bytes: None,
        memory_percent: None,
        working_dir: None,
        environment: env
            .iter()
            .map(|(k, v)| (String::from(*k), String::from(*v)))
            .collect(),
        children: Vec::new(),
        memory: None,
        io: None,
        open_files: Vec::new(),
        fd_count: None,
        fd_limit: None,
    };
    let env_ctx = |details: &runquiry_core::ProcessDetails| {
        runquiry_core::warnings(&runquiry_core::WarningsContext {
            target: &t,
            sockets: &sockets,
            details: Some(details),
            service: None,
            restart_count: 0,
            source_type: SourceType::Shell,
            is_windows: false,
            now: SystemTime::UNIX_EPOCH,
        })
    };

    let d = details_with(&[
        ("LD_PRELOAD", "/opt/evil.so"),
        ("DYLD_INSERT_LIBRARIES", "/x"),
        ("DYLD_FRAMEWORK_PATH", "/y"),
    ]);
    let warnings = env_ctx(&d);
    assert_eq!(
        kinds(&warnings),
        vec![WarningKind::SuspiciousEnv, WarningKind::SuspiciousEnv],
        "两条规则各一条，LD_PRELOAD 在前"
    );
    assert_eq!(
        warnings[0].message(),
        "Process sets LD_PRELOAD (potential library injection)"
    );
    assert_eq!(
        warnings[1].message(),
        "Process sets DYLD_* variables (potential library injection): DYLD_FRAMEWORK_PATH, DYLD_INSERT_LIBRARIES",
        "DYLD 命中键按 key 排序"
    );

    // 空值跳过；普通变量不告警。
    let d = details_with(&[("LD_PRELOAD", ""), ("PATH", "/usr/bin")]);
    assert!(env_ctx(&d).is_empty());
}

/// 无数据输入（无 sockets、无 details、无 service）→ 无告警基线。
#[test]
fn warnings_minimal_context_yields_no_warnings() {
    let t = target();
    let sockets: Vec<SocketEntry> = Vec::new();
    let ctx = runquiry_core::WarningsContext {
        target: &t,
        sockets: &sockets,
        details: None,
        service: None,
        restart_count: 0,
        source_type: SourceType::Shell,
        is_windows: false,
        now: SystemTime::UNIX_EPOCH,
    };
    assert!(runquiry_core::warnings(&ctx).is_empty());
}
