//! B1 契约门：cgroup 容器解析、systemd 单元解析、来源模型与目标解析容器
//! 的已实现纯函数契约测试。完整分析管线测试在契约冻结后的下一阶段补齐。

use runquiry_core::{
    ContainerContext, HealthStatus, Resolution, Source, SourceType, detect_container_from_cgroup,
    detect_lxc_runtime, find_long_hex_id, short_id, systemd_unit_from_cgroup,
};

mod support;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// 64 位十六进制长 ID（合成值，非真实容器 ID）。
const LONG_HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[test]
fn pipeline_cgroup_docker_scope_and_path_patterns_yield_container_context() -> TestResult {
    // cgroup v2 scope 模式：docker-<64hex>.scope。
    let v2 = format!("0::/system.slice/docker-{LONG_HEX}.scope\n");
    let ctx = detect_container_from_cgroup(&v2)
        .ok_or_else(|| String::from("docker scope 应识别出容器上下文"))?;
    assert_eq!(ctx.runtime(), "docker");
    assert_eq!(ctx.container_id(), LONG_HEX);

    // cgroup v1 路径模式：/docker/<64hex>。
    let v1 = format!("1:name=systemd:/docker/{LONG_HEX}\n");
    let ctx = detect_container_from_cgroup(&v1)
        .ok_or_else(|| String::from("docker 路径应识别出容器上下文"))?;
    assert_eq!(ctx.runtime(), "docker");
    assert_eq!(ctx.container_id(), LONG_HEX);
    Ok(())
}

#[test]
fn pipeline_cgroup_podman_and_libpod_share_podman_runtime() -> TestResult {
    let scope = format!("0::/user.slice/user-1000.slice/libpod-{LONG_HEX}.scope\n");
    let ctx = detect_container_from_cgroup(&scope)
        .ok_or_else(|| String::from("libpod scope 应识别出容器上下文"))?;
    assert_eq!(ctx.runtime(), "podman");
    assert_eq!(ctx.container_id(), LONG_HEX);

    let path = format!("11:pids:/libpod/{LONG_HEX}\n");
    let ctx = detect_container_from_cgroup(&path)
        .ok_or_else(|| String::from("libpod 路径应识别出容器上下文"))?;
    assert_eq!(ctx.runtime(), "podman");
    assert_eq!(ctx.container_id(), LONG_HEX);
    Ok(())
}

#[test]
fn pipeline_cgroup_kubepods_uses_crictl_runtime_and_long_hex_id() -> TestResult {
    let content = format!("0::/kubepods.slice/kubepods-besteffort.slice/{LONG_HEX}\n");
    let ctx = detect_container_from_cgroup(&content)
        .ok_or_else(|| String::from("kubepods 应识别出容器上下文"))?;
    assert_eq!(ctx.runtime(), "crictl");
    assert_eq!(ctx.container_id(), LONG_HEX);
    Ok(())
}

#[test]
fn pipeline_cgroup_containerd_uses_nerdctl_runtime_and_long_hex_id() -> TestResult {
    let content = format!("0::/containerd/fxt/{LONG_HEX}\n");
    let ctx = detect_container_from_cgroup(&content)
        .ok_or_else(|| String::from("containerd 应识别出容器上下文"))?;
    assert_eq!(ctx.runtime(), "nerdctl");
    assert_eq!(ctx.container_id(), LONG_HEX);
    Ok(())
}

#[test]
fn pipeline_cgroup_colima_yields_scope_id_or_default_without_id() -> TestResult {
    let with_id = "0::/user.slice/colima-cafe1234.scope\n";
    let ctx = detect_container_from_cgroup(with_id)
        .ok_or_else(|| String::from("colima scope 应识别出容器上下文"))?;
    assert_eq!(ctx.runtime(), "colima");
    assert_eq!(ctx.container_id(), "cafe1234");

    // witr 对无 ID 的 colima 记 "colima: default"：上下文存在但无 ID。
    let default = "0::/colima/default\n";
    let ctx = detect_container_from_cgroup(default)
        .ok_or_else(|| String::from("colima 默认实例应识别出容器上下文"))?;
    assert_eq!(ctx.runtime(), "colima");
    assert_eq!(ctx.container_id(), "");
    Ok(())
}

#[test]
fn pipeline_cgroup_lxc_payload_yields_lxc_runtime_with_payload_name() -> TestResult {
    let content = "0::/lxc.payload.fxt-container/user/0\n";
    let ctx = detect_container_from_cgroup(content)
        .ok_or_else(|| String::from("lxc.payload 应识别出容器上下文"))?;
    assert_eq!(ctx.runtime(), "lxc");
    assert_eq!(ctx.container_id(), "fxt-container");
    Ok(())
}

#[test]
fn pipeline_cgroup_systemd_service_is_not_a_container() {
    assert_eq!(
        detect_container_from_cgroup("0::/system.slice/nginx.service\n"),
        None,
        "systemd 单元路径不得误判为容器"
    );
    assert_eq!(detect_container_from_cgroup(""), None);
    assert_eq!(
        detect_container_from_cgroup("0::/user.slice/user-1000.slice\n"),
        None
    );
}

#[test]
fn pipeline_long_hex_id_extraction_follows_witr_semantics() {
    assert_eq!(
        find_long_hex_id(&format!("prefix-{LONG_HEX}-suffix")),
        Some(LONG_HEX.to_string())
    );
    // 大写十六进制同样命中（witr 逐字符校验 0-9a-fA-F）。
    let upper = LONG_HEX.to_uppercase();
    assert_eq!(find_long_hex_id(&upper), Some(upper.clone()));
    // 不足 64 位不命中。
    assert_eq!(find_long_hex_id("0123456789abcdef"), None);
    // 超长十六进制串取首个 64 字符窗口。
    let long65 = format!("{LONG_HEX}f");
    assert_eq!(find_long_hex_id(&long65), Some(LONG_HEX.to_string()));
    // 中途混入非十六进制字符打断窗口。
    let broken = format!("{LONG_HEX}g{LONG_HEX}");
    assert_eq!(find_long_hex_id(&broken), Some(LONG_HEX.to_string()));
    assert_eq!(find_long_hex_id(""), None);
}

#[test]
fn pipeline_short_id_truncates_to_twelve_characters() {
    assert_eq!(short_id(LONG_HEX), &LONG_HEX[..12]);
    assert_eq!(short_id("short-id"), "short-id");
    assert_eq!(short_id("exactly12chr"), "exactly12chr");
}

#[test]
fn pipeline_lxc_runtime_detection_matches_witr_ancestor_commands() {
    // witr `TestDetectLXCRuntime`：命令名精确匹配（incusd 而非 incus）。
    assert_eq!(detect_lxc_runtime("incusd"), "incus");
    assert_eq!(detect_lxc_runtime("lxd"), "lxd");
    assert_eq!(detect_lxc_runtime("lxc-start"), "lxc");
    assert_eq!(detect_lxc_runtime("unrelated"), "lxc");
    assert_eq!(detect_lxc_runtime(""), "lxc");
}

#[test]
fn pipeline_systemd_unit_from_cgroup_parses_v1_v2_and_scope() {
    // cgroup v2：controllers 为空。
    assert_eq!(
        systemd_unit_from_cgroup("0::/system.slice/nginx.service\n"),
        Some(String::from("nginx.service"))
    );
    // cgroup v1：controller 含 name=systemd。
    assert_eq!(
        systemd_unit_from_cgroup("1:name=systemd:/system.slice/nginx.service\n"),
        Some(String::from("nginx.service"))
    );
    // 非 systemd controller 的 v1 行被跳过。
    assert_eq!(
        systemd_unit_from_cgroup("11:pids:/system.slice/nginx.service\n"),
        None
    );
    // witr 同样接受 .scope 结尾（用户会话作用域）。
    assert_eq!(
        systemd_unit_from_cgroup(
            "0::/user.slice/user-1000.slice/user@1000.service/app.slice/app-fxt.scope\n"
        ),
        Some(String::from("app-fxt.scope"))
    );
    // 嵌套路径取最深命中单元。
    assert_eq!(
        systemd_unit_from_cgroup("0::/system.slice/foo.service/subgroup\n"),
        Some(String::from("foo.service"))
    );
    // 无单元命中时返回 None。
    assert_eq!(systemd_unit_from_cgroup("0::/\n"), None);
    assert_eq!(systemd_unit_from_cgroup(""), None);
}

#[test]
fn pipeline_container_context_reader_and_healthcheck_default() {
    let ctx = ContainerContext::new(String::from(LONG_HEX), String::from("docker"), None);
    assert_eq!(ctx.container_id(), LONG_HEX);
    assert_eq!(ctx.runtime(), "docker");
    assert_eq!(ctx.healthcheck(), None);
}

#[test]
fn pipeline_source_construction_readers_and_detail_ordering() {
    let source = Source::new(SourceType::Systemd)
        .with_name(String::from("nginx.service"))
        .with_description(String::from("A high performance web server"))
        .with_unit_file(String::from("/usr/lib/systemd/system/nginx.service"))
        .with_detail(String::from("NRestarts"), String::from("7"));

    assert_eq!(source.source_type(), SourceType::Systemd);
    assert_eq!(source.name(), Some("nginx.service"));
    assert_eq!(source.description(), Some("A high performance web server"));
    assert_eq!(
        source.unit_file(),
        Some("/usr/lib/systemd/system/nginx.service")
    );
    // details 为有序键值：core 只透传平台采集顺序，不重排。
    assert_eq!(
        source.details(),
        &[(String::from("NRestarts"), String::from("7"))]
    );
}

#[test]
fn pipeline_source_unknown_is_the_never_blank_fallback() {
    // parity：来源识别永不返回空（Detect 末尾兜底 SourceUnknown）。
    let fallback = Source::unknown();
    assert_eq!(fallback.source_type(), SourceType::Unknown);
    assert_eq!(fallback.name(), None);
    assert_eq!(fallback.details(), &[] as &[(String, String)]);
}

#[test]
fn pipeline_source_type_codes_match_witr_strings() {
    assert_eq!(SourceType::Container.code(), "container");
    assert_eq!(SourceType::Ssh.code(), "ssh");
    assert_eq!(SourceType::Shell.code(), "shell");
    assert_eq!(SourceType::Systemd.code(), "systemd");
    assert_eq!(SourceType::Launchd.code(), "launchd");
    assert_eq!(SourceType::BsdRc.code(), "bsdrc");
    assert_eq!(SourceType::Supervisor.code(), "supervisor");
    assert_eq!(SourceType::Cron.code(), "cron");
    assert_eq!(SourceType::WindowsService.code(), "windows_service");
    assert_eq!(SourceType::Init.code(), "init");
    assert_eq!(SourceType::Unknown.code(), "unknown");
}

#[test]
fn pipeline_resolution_keeps_all_candidates_without_auto_selection() {
    let unique: Resolution<u32> = Resolution::Unique(42);
    assert_eq!(unique.count(), 1);
    assert_eq!(unique.into_candidates(), vec![42]);

    // 多结果不自动选择：Ambiguous 携带完整、稳定排序的候选集合。
    let ambiguous: Resolution<u32> = Resolution::Ambiguous(vec![2, 7, 11]);
    assert_eq!(ambiguous.count(), 3);
    assert_eq!(ambiguous.into_candidates(), vec![2, 7, 11]);
}

#[test]
fn pipeline_additive_summary_fields_default_when_absent_in_json() -> TestResult {
    // 序列化兼容红线：缺少新增字段的旧条目必须按默认值读取。
    let legacy = r#"{
        "identity": {"pid": 4242, "start_time": null, "executable": null},
        "parent_pid": null,
        "command": "fxt-daemon",
        "command_line": null,
        "user": null
    }"#;
    let summary: runquiry_core::ProcessSummary = serde_json::from_str(legacy)?;
    assert_eq!(summary.health, HealthStatus::Unknown);
    assert_eq!(summary.container, None);
    assert!(!summary.exe_deleted);
    assert!(summary.capabilities.is_empty());
    Ok(())
}

#[test]
fn pipeline_additive_summary_fields_round_trip() -> TestResult {
    let summary = runquiry_core::ProcessSummary {
        identity: runquiry_core::ProcessIdentity::new(runquiry_core::Pid::new(7)?, None, None),
        parent_pid: None,
        command: String::from("fxt-daemon"),
        command_line: None,
        user: None,
        health: HealthStatus::HighCpu,
        container: Some(ContainerContext::new(
            String::from(LONG_HEX),
            String::from("docker"),
            Some(runquiry_core::HealthcheckStatus::Absent),
        )),
        exe_deleted: true,
        capabilities: vec![String::from("CAP_SYS_ADMIN")],
    };
    let text = serde_json::to_string(&summary)?;
    assert!(
        text.contains(r#""health":"high-cpu""#),
        "序列化标签与 witr 一致：{text}"
    );
    assert!(
        text.contains(r#""exe_deleted":true"#),
        "新增字段参与序列化：{text}"
    );
    let back: runquiry_core::ProcessSummary = serde_json::from_str(&text)?;
    assert_eq!(back.health, HealthStatus::HighCpu);
    assert!(back.exe_deleted);
    assert_eq!(back.capabilities, vec![String::from("CAP_SYS_ADMIN")]);
    assert_eq!(
        back.container
            .as_ref()
            .and_then(runquiry_core::ContainerContext::healthcheck),
        Some(runquiry_core::HealthcheckStatus::Absent)
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// B1 完整实现：祖先链、来源识别与分析管线（parity §4/§5）
// ---------------------------------------------------------------------------

use std::time::Duration;

use runquiry_core::{
    AnalysisPorts, DiagnosticCode, InspectError, Inspection, Pid, ProcessDetails, ProcessIdentity,
    SourceEvidence, analyze, resolve_ancestry,
};

use crate::support::collectors::{
    FakeContainers, FakeDetails, FakeEvidence, FakeInventory, FakeNetwork, FakeProcessLocks,
    captured_at, summary,
};

/// 祖先链：root→target 顺序（完整链 1←3←5）。
#[test]
fn pipeline_ancestry_returns_root_to_target_order() -> TestResult {
    let now = captured_at();
    let mut reads = std::collections::HashMap::new();
    reads.insert(1, summary(1, Some(0), "systemd", None));
    reads.insert(3, summary(3, Some(1), "sshd", None));
    reads.insert(5, summary(5, Some(3), "bash", None));
    let read =
        move |pid: Pid| Ok::<_, runquiry_core::DiagnosticIssue>(reads.get(&pid.get()).cloned());
    let inspection = resolve_ancestry(Pid::new(5)?, now, &read)?;
    let chain = inspection
        .data()
        .ok_or_else(|| String::from("应有祖先链"))?;
    assert_eq!(chain.len(), 3);
    assert_eq!(chain[0].identity.pid(), Pid::new(1)?, "root 在前");
    assert_eq!(chain[2].identity.pid(), Pid::new(5)?, "target 在尾");
    // PID 1 到达即止。
    assert_eq!(chain[0].command, "systemd");
    Ok(())
}

/// 单跳失败就地截断（父进程读不到 → 链截为目标与已读到部分）；
/// 目标本身不可读 → NotFound（进程已退出）。
#[test]
fn pipeline_ancestry_truncates_on_partial_read_and_fails_when_target_missing() -> TestResult {
    let now = captured_at();
    let mut reads = std::collections::HashMap::new();
    reads.insert(5, summary(5, Some(3), "bash", None)); // 父进程 3 读不到
    let snapshot_reads = reads.clone();
    let read = move |pid: Pid| {
        Ok::<_, runquiry_core::DiagnosticIssue>(snapshot_reads.get(&pid.get()).cloned())
    };
    let inspection = resolve_ancestry(Pid::new(5)?, now, &read)?;
    let chain = inspection
        .data()
        .ok_or_else(|| String::from("部分成功应保留数据"))?;
    assert_eq!(chain.len(), 1, "单跳失败就地截断");
    assert_eq!(chain[0].identity.pid(), Pid::new(5)?);
    // 截断可区分（根计划完成标准）：清单缺失的祖先记一条诊断。
    assert_eq!(inspection.issues.len(), 1, "缺失祖先记诊断");
    assert_eq!(inspection.issues[0].code().code(), "unknown");

    // 权限类失败同样可区分：Err 通道的诊断原样进入 issues。
    let only_target = reads;
    let read = move |pid: Pid| {
        if pid.get() == 3 {
            Err(runquiry_core::DiagnosticIssue::new(
                runquiry_core::DiagnosticCode::PermissionDenied,
                String::from("合成场景：父进程不可读"),
            ))
        } else {
            Ok(only_target.get(&pid.get()).cloned())
        }
    };
    let inspection = resolve_ancestry(Pid::new(5)?, now, &read)?;
    assert_eq!(inspection.issues[0].code().code(), "permission_denied");

    let nothing: std::collections::HashMap<u32, runquiry_core::ProcessSummary> =
        std::collections::HashMap::new();
    let read =
        move |pid: Pid| Ok::<_, runquiry_core::DiagnosticIssue>(nothing.get(&pid.get()).cloned());
    let err = resolve_ancestry(Pid::new(5)?, now, &read)
        .err()
        .ok_or_else(|| String::from("目标不可读应失败"))?;
    assert_eq!(err.code(), "not_found", "进程已退出 → NotFound");
    Ok(())
}

/// 环检测：PPID 回指已访问 PID 时截断，不发散（witr seen map loop protection）。
#[test]
fn pipeline_ancestry_breaks_pid_cycles() -> TestResult {
    let now = captured_at();
    let mut reads = std::collections::HashMap::new();
    reads.insert(9, summary(9, Some(10), "a", None));
    reads.insert(10, summary(10, Some(9), "b", None));
    let read =
        move |pid: Pid| Ok::<_, runquiry_core::DiagnosticIssue>(reads.get(&pid.get()).cloned());
    let inspection = resolve_ancestry(Pid::new(9)?, now, &read)?;
    let chain = inspection
        .data()
        .ok_or_else(|| String::from("环应有截断结果"))?;
    assert_eq!(chain.len(), 2, "9→10 后 10 的父 9 已访问，截断");
    assert!(inspection.issues.is_empty(), "环截断是正常终止，不记诊断");
    Ok(())
}

/// 来源识别 fixture 对照（parity §5 优先级链与各来源判据）。
fn detect_for(
    ancestry: &[runquiry_core::ProcessSummary],
    evidence: &SourceEvidence,
) -> runquiry_core::Source {
    runquiry_core::detect_source(ancestry, evidence)
}

#[test]
fn pipeline_source_detection_priority_and_branches() -> TestResult {
    let mut cgroup = SourceEvidence::default();
    cgroup.cgroup_by_pid.push((
        Pid::new(5)?,
        String::from("0::/system.slice/docker-abc.scope\n"),
    ));
    let ancestry = vec![
        summary(1, Some(0), "systemd", None),
        summary(5, Some(1), "python", None),
    ];
    let src = detect_for(&ancestry, &cgroup);
    assert_eq!(
        src.source_type(),
        SourceType::Container,
        "容器优先于其他来源"
    );
    assert_eq!(src.name(), Some("docker"));

    // cgroup 与 sshd 同时存在 → 容器胜出（优先级链 container → ssh）。
    Ok(())
}

#[test]
fn pipeline_source_detection_container_runtimes_and_snap_flatpak() -> TestResult {
    let cases = [
        ("0::/system.slice/libpod-abc.scope\n", "podman"),
        ("0::/kubepods.slice/abc\n", "kubernetes"),
        ("0::/colima-cafe.scope\n", "colima"),
        ("0::/containerd/abc\n", "containerd"),
    ];
    for (cgroup, name) in cases {
        let mut evidence = SourceEvidence::default();
        evidence
            .cgroup_by_pid
            .push((Pid::new(5)?, String::from(cgroup)));
        let ancestry = vec![summary(5, Some(1), "worker", None)];
        let src = detect_for(&ancestry, &evidence);
        assert_eq!(src.source_type(), SourceType::Container, "{cgroup}");
        assert_eq!(src.name(), Some(name), "{cgroup}");
    }

    // lxc.payload：按祖先命令精化 runtime（incusd → incus）。
    let mut evidence = SourceEvidence::default();
    evidence
        .cgroup_by_pid
        .push((Pid::new(5)?, String::from("0::/lxc.payload.fxt-box\n")));
    let ancestry = vec![
        summary(3, Some(1), "incusd", None),
        summary(5, Some(3), "bash", None),
    ];
    let src = detect_for(&ancestry, &evidence);
    assert_eq!(src.name(), Some("incus"));

    // Snap / Flatpak：目标进程环境变量判定（cgroup 判不出时）。
    let mut snap = SourceEvidence::default();
    snap.env_by_pid.push((
        Pid::new(5)?,
        vec![(String::from("SNAP_NAME"), String::from("fxt-app"))],
    ));
    let ancestry = vec![summary(5, Some(1), "fxt-app", None)];
    assert_eq!(detect_for(&ancestry, &snap).name(), Some("snap"));
    let mut flatpak = SourceEvidence::default();
    flatpak.env_by_pid.push((
        Pid::new(5)?,
        vec![(String::from("FLATPAK_ID"), String::from("org.fxt.App"))],
    ));
    assert_eq!(detect_for(&ancestry, &flatpak).name(), Some("flatpak"));
    Ok(())
}

#[test]
fn pipeline_source_detection_ssh_and_shell_and_multiplexer() -> TestResult {
    // SSH：祖先链（排除目标自身）有 sshd 且链长 ≥2；连接详情从 env 回溯。
    let mut evidence = SourceEvidence::default();
    evidence.env_by_pid.push((
        Pid::new(5)?,
        vec![(String::from("SSH_CLIENT"), String::from("10.0.0.9 2222 22"))],
    ));
    let ancestry = vec![
        summary(3, Some(1), "sshd: fxt", None),
        summary(5, Some(3), "bash", None),
    ];
    let src = detect_for(&ancestry, &evidence);
    assert_eq!(src.source_type(), SourceType::Ssh);
    assert_eq!(src.name(), Some("sshd"));
    let description = src
        .description()
        .ok_or_else(|| String::from("SSH 描述应组装"))?;
    assert!(
        description.contains("10.0.0.9"),
        "描述含远端 IP：{description}"
    );

    // 链长 <2 不判定 SSH。
    let ancestry = vec![summary(5, Some(1), "sshd", None)];
    let src = detect_for(&ancestry, &SourceEvidence::default());
    assert_ne!(src.source_type(), SourceType::Ssh);

    // Shell：祖先为 bash（detectShell 扫描排除目标自身，故 shell 须在祖先链上）。
    let ancestry = vec![
        summary(1, Some(0), "systemd", None),
        summary(3, Some(1), "bash", None),
        summary(5, Some(3), "fxt-job", None),
    ];
    let src = detect_for(&ancestry, &SourceEvidence::default());
    assert_eq!(src.source_type(), SourceType::Shell);
    assert_eq!(src.name(), Some("bash"));

    let ancestry = vec![
        summary(7, Some(1), "tmux: fxt-session", None),
        summary(9, Some(7), "python3", None),
        summary(5, Some(9), "fxt-server", None),
    ];
    let mut evidence = SourceEvidence::default();
    evidence.env_by_pid.push((
        Pid::new(5)?,
        vec![(
            String::from("TMUX"),
            String::from("/tmp/tmux-1000/fxt-session,123,0"),
        )],
    ));
    let src = detect_for(&ancestry, &evidence);
    assert_eq!(src.source_type(), SourceType::Shell);
    assert_eq!(
        src.name(),
        Some("python3"),
        "userTool 命中（python 前缀族）"
    );
    let description = src.description().unwrap_or_default();
    assert!(
        description.contains("fxt-session"),
        "tmux 会话富化：{description}"
    );
    Ok(())
}

#[test]
fn pipeline_source_detection_systemd_supervisor_cron_init_unknown() -> TestResult {
    // systemd：systemd_running 前提 + 链含 PID 1 + 单元名来自 cgroup；
    // D-Bus 富化键值原样透传进 details。
    let mut evidence = SourceEvidence {
        systemd_running: true,
        ..SourceEvidence::default()
    };
    evidence
        .cgroup_by_pid
        .push((Pid::new(5)?, String::from("0::/system.slice/fxt.service\n")));
    evidence
        .systemd_details
        .push((String::from("NRestarts"), String::from("7")));
    evidence
        .systemd_details
        .push((String::from("Description"), String::from("Fxt service")));
    let ancestry = vec![
        summary(1, Some(0), "systemd", None),
        summary(5, Some(1), "fxt", None),
    ];
    let src = detect_for(&ancestry, &evidence);
    assert_eq!(src.source_type(), SourceType::Systemd);
    assert_eq!(src.name(), Some("fxt.service"));
    assert_eq!(
        src.details(),
        &[
            (String::from("NRestarts"), String::from("7")),
            (String::from("Description"), String::from("Fxt service")),
        ]
    );

    // supervisor：祖先 supervisord（systemd 未运行时不走 systemd 分支）。
    // 注意 witr 按祖先链 root→target 扫描：链上若先出现 "init"/"systemd"
    // 等名单键会先命中，故此处用 supervisord 直系祖先链。
    let ancestry = vec![
        summary(3, Some(1), "supervisord", None),
        summary(5, Some(3), "fxt-worker", None),
    ];
    let src = detect_for(&ancestry, &SourceEvidence::default());
    assert_eq!(src.source_type(), SourceType::Supervisor);
    assert_eq!(src.name(), Some("supervisord"));

    // cron：祖先 crond。
    let ancestry = vec![
        summary(1, Some(0), "crond", None),
        summary(5, Some(1), "job", None),
    ];
    assert_eq!(
        detect_for(&ancestry, &SourceEvidence::default()).source_type(),
        SourceType::Cron
    );

    // init：根进程 PID 1 且链中无 shell、根命令不在 supervisor 名单内
    // → 来源 init，名字取 PID 1 命令名（witr init 兜底）。
    let ancestry = vec![
        summary(1, Some(0), "fxt-sysvinit", None),
        summary(5, Some(1), "fxt", None),
    ];
    let src = detect_for(&ancestry, &SourceEvidence::default());
    assert_eq!(src.source_type(), SourceType::Init);
    assert_eq!(src.name(), Some("fxt-sysvinit"));

    // 全部未命中 → unknown（永不返回空）。
    let ancestry = vec![summary(5, Some(1), "fxt-daemon", None)];
    let src = detect_for(&ancestry, &SourceEvidence::default());
    assert_eq!(src.source_type(), SourceType::Unknown);
    Ok(())
}

/// 分析管线：按 witr `AnalyzePID` 顺序组合，单个采集器失败不抹掉已有数据。
#[test]
fn pipeline_analyze_composes_collectors_and_keeps_data_on_partial_failure() -> TestResult {
    let now = captured_at();
    let target = summary(5, Some(3), "fxt-daemon", Some("fxt-daemon --serve"));
    let parent = summary(3, Some(1), "supervisord", None);
    let root = summary(1, Some(0), "systemd", None);
    let ancestry = vec![root, parent, target.clone()];

    let inventory = FakeInventory::new(ancestry);
    let details = ProcessDetails {
        identity: target.identity.clone(),
        cpu_percent: Some(1.5),
        memory_rss_bytes: Some(4096),
        memory_percent: None,
        working_dir: Some(std::path::PathBuf::from("/opt/runquiry-fixtures/var")),
        environment: Vec::new(),
        children: Vec::new(),
        memory: None,
        io: None,
        open_files: Vec::new(),
        fd_count: None,
        fd_limit: None,
    };
    let details = FakeDetails::ok(&target.identity, details);
    let network = FakeNetwork {
        result: Inspection::complete(Vec::new()),
    };
    let containers = FakeContainers {
        containers: Vec::new(),
    };
    let evidence = FakeEvidence {
        evidence: SourceEvidence::default(),
    };
    let locks = FakeProcessLocks { locks: Vec::new() };
    let ports = AnalysisPorts {
        inventory: &inventory,
        details: &details,
        network: &network,
        containers: &containers,
        evidence: &evidence,
        healthcheck: None,
        process_locks: Some(&locks),
    };

    let inspection = analyze(&target.identity, &ports, now, false)?;
    let analysis = inspection
        .data()
        .ok_or_else(|| String::from("正常场景应有完整数据"))?;
    assert_eq!(
        analysis.resolved_target, "fxt-daemon",
        "ResolvedTarget 取链尾 Command"
    );
    assert_eq!(analysis.ancestry.len(), 3, "root→target");
    assert_eq!(
        analysis.source.source_type(),
        SourceType::Supervisor,
        "root systemd（未运行 systemd）命中 supervisor 名单（witr 顺序）"
    );
    assert_eq!(analysis.source.name(), Some("systemd service"));
    Ok(())
}

/// 部分成功：socket 采集失败只追加诊断；详情权限失败降级（资源详情缺失但
/// 其余数据保留）；子进程快照失败静默降级为空（无诊断、无告警影响）。
#[test]
fn pipeline_analyze_degrades_on_single_collector_failures() -> TestResult {
    let now = captured_at();
    let target = summary(5, Some(3), "fxt-daemon", Some("fxt-daemon --serve"));
    let parent = summary(3, Some(1), "bash", None);
    let ancestry = vec![parent, target.clone()];
    let inventory = FakeInventory::new(ancestry);
    let permission = InspectError::PermissionDenied {
        subject: String::from("进程详情（合成场景）"),
    };
    let details = FakeDetails::err(&target.identity, permission);
    let network = FakeNetwork {
        result: Inspection::failed(vec![runquiry_core::DiagnosticIssue::new(
            DiagnosticCode::Timeout,
            String::from("合成超时"),
        )]),
    };
    let containers = FakeContainers {
        containers: Vec::new(),
    };
    let evidence = FakeEvidence {
        evidence: SourceEvidence::default(),
    };
    let ports = AnalysisPorts {
        inventory: &inventory,
        details: &details,
        network: &network,
        containers: &containers,
        evidence: &evidence,
        healthcheck: None,
        process_locks: None,
    };

    let inspection = analyze(&target.identity, &ports, now, false)?;
    let analysis = inspection
        .data()
        .ok_or_else(|| String::from("单采集器失败不得抹掉已有数据"))?;
    assert_eq!(analysis.ancestry.len(), 2, "祖先链保留");
    assert!(inspection.has_issues(), "socket 超时与详情权限失败须记诊断");
    assert!(analysis.sockets.is_empty(), "socket 采集失败无数据");
    assert!(analysis.details.is_none(), "详情权限失败降级为 None");
    Ok(())
}

/// PID 复用 vs 进程退出：details 返回 `ProcessChanged` → 整体失败并区分于
/// `NotFound`（用 `ProcessIdentity::same_process` 判定，parity §9 身份语义）。
#[test]
fn pipeline_analyze_distinguishes_pid_reuse_from_process_exit() -> TestResult {
    let now = captured_at();
    let target = summary(5, Some(1), "fxt-daemon", None);
    let ancestry = vec![target.clone()];
    let make_ports = |outcome: Result<ProcessDetails, InspectError>| {
        let inventory = FakeInventory::new(ancestry.clone());
        let details = FakeDetails {
            baseline: target.identity.clone(),
            outcome,
        };
        (
            inventory,
            details,
            FakeNetwork {
                result: Inspection::complete(Vec::new()),
            },
            FakeContainers {
                containers: Vec::new(),
            },
            FakeEvidence {
                evidence: SourceEvidence::default(),
            },
        )
    };

    // PID 复用：身份比对失败（start_time 不同）→ ProcessChanged。
    let reused_identity =
        ProcessIdentity::new(Pid::new(5)?, Some(now + Duration::from_secs(1)), None);
    let (inventory, details, network, containers, evidence) =
        make_ports(Err(InspectError::ProcessChanged {
            identity: reused_identity,
        }));
    let ports = AnalysisPorts {
        inventory: &inventory,
        details: &details,
        network: &network,
        containers: &containers,
        evidence: &evidence,
        healthcheck: None,
        process_locks: None,
    };
    let err = analyze(&target.identity, &ports, now, false)
        .err()
        .ok_or_else(|| String::from("PID 复用应整体失败"))?;
    assert_eq!(err.code(), "process_changed");

    // 进程退出：NotFound → 整体失败且错误码可区分。
    let (inventory, details, network, containers, evidence) =
        make_ports(Err(InspectError::NotFound {
            subject: String::from("PID 5"),
        }));
    let ports = AnalysisPorts {
        inventory: &inventory,
        details: &details,
        network: &network,
        containers: &containers,
        evidence: &evidence,
        healthcheck: None,
        process_locks: None,
    };
    let err = analyze(&target.identity, &ports, now, false)
        .err()
        .ok_or_else(|| String::from("进程退出应整体失败"))?;
    assert_eq!(err.code(), "not_found");
    Ok(())
}

/// 容器健康检查补全：探针结果写回目标容器上下文（仅容器进程触发）；
/// 空详情（合成身份对齐目标）。
fn empty_details(target: &runquiry_core::ProcessSummary) -> ProcessDetails {
    ProcessDetails {
        identity: target.identity.clone(),
        cpu_percent: None,
        memory_rss_bytes: None,
        memory_percent: None,
        working_dir: None,
        environment: Vec::new(),
        children: Vec::new(),
        memory: None,
        io: None,
        open_files: Vec::new(),
        fd_count: None,
        fd_limit: None,
    }
}

/// systemd 来源：`NRestarts` 解析为 `restart_count`（parity：`AnalyzePID`
/// 的 `NRestarts` 分支）。
#[test]
fn pipeline_analyze_parses_systemd_restart_count() -> TestResult {
    let now = captured_at();
    let mut target = summary(5, Some(1), "fxt", Some("fxt --serve"));
    target.container = None;
    let root = summary(1, Some(0), "systemd", None);
    let ancestry = vec![root, target.clone()];
    let mut evidence = SourceEvidence {
        systemd_running: true,
        ..SourceEvidence::default()
    };
    evidence
        .cgroup_by_pid
        .push((Pid::new(5)?, String::from("0::/system.slice/fxt.service\n")));
    evidence
        .systemd_details
        .push((String::from("NRestarts"), String::from("9")));

    let inventory = FakeInventory::new(ancestry);
    let details = FakeDetails::ok(&target.identity, empty_details(&target));
    let network = FakeNetwork {
        result: Inspection::complete(Vec::new()),
    };
    let containers = FakeContainers {
        containers: Vec::new(),
    };
    let evidence = FakeEvidence { evidence };
    let ports = AnalysisPorts {
        inventory: &inventory,
        details: &details,
        network: &network,
        containers: &containers,
        evidence: &evidence,
        healthcheck: None,
        process_locks: None,
    };
    let inspection = analyze(&target.identity, &ports, now, false)?;
    let analysis = inspection.data().ok_or_else(|| String::from("应有数据"))?;
    assert_eq!(analysis.source.source_type(), SourceType::Systemd);
    assert_eq!(analysis.restart_count, 9, "systemd NRestarts 解析");
    Ok(())
}

/// LXC 系容器来源按祖先命令改写运行时标签（parity：AnalyzePID 的标签一致
/// 性改写；incusd → incus）。
#[test]
fn pipeline_analyze_rewrites_lxc_runtime_from_ancestry() -> TestResult {
    let now = captured_at();
    let mut lxc_target = summary(5, Some(3), "app", None);
    lxc_target.container = Some(runquiry_core::ContainerContext::new(
        String::from("fxt-box"),
        String::from("lxc"),
        None,
    ));
    let incus = summary(3, Some(1), "incusd", None);
    let lxc_ancestry = vec![incus, lxc_target.clone()];
    let lxc_evidence = SourceEvidence {
        cgroup_by_pid: vec![(Pid::new(5)?, String::from("0::/lxc.payload.fxt-box\n"))],
        ..SourceEvidence::default()
    };
    let inventory = FakeInventory::new(lxc_ancestry);
    let details = FakeDetails::ok(&lxc_target.identity, empty_details(&lxc_target));
    let network = FakeNetwork {
        result: Inspection::complete(Vec::new()),
    };
    let containers = FakeContainers {
        containers: Vec::new(),
    };
    let evidence = FakeEvidence {
        evidence: lxc_evidence,
    };
    let ports = AnalysisPorts {
        inventory: &inventory,
        details: &details,
        network: &network,
        containers: &containers,
        evidence: &evidence,
        healthcheck: None,
        process_locks: None,
    };
    let inspection = analyze(&lxc_target.identity, &ports, now, false)?;
    let analysis = inspection.data().ok_or_else(|| String::from("应有数据"))?;
    assert_eq!(analysis.source.source_type(), SourceType::Container);
    assert_eq!(analysis.source.name(), Some("incus"));
    let rewritten = analysis
        .target
        .container
        .as_ref()
        .ok_or_else(|| String::from("容器上下文应保留"))?;
    assert_eq!(rewritten.runtime(), "incus");
    Ok(())
}
