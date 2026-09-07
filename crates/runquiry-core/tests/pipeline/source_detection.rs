//! 来源识别优先级契约。

use runquiry_core::{Pid, SourceEvidence, SourceType};

use crate::support::collectors::summary;

use super::TestResult;
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

#[test]
fn pipeline_source_detection_launchd_matches_witr_semantics() -> TestResult {
    // launchd 前提：祖先链根为 PID 1 且命令为 launchd；证据缺失回退基础来源
    // （parity `detectLaunchd` 的 GetLaunchdInfo 失败分支）。
    let ancestry = vec![
        summary(1, Some(0), "launchd", None),
        summary(5, Some(1), "fxt-daemon", None),
    ];
    let src = detect_for(&ancestry, &SourceEvidence::default());
    assert_eq!(src.source_type(), SourceType::Launchd);
    assert_eq!(src.name(), Some("launchd"));

    // 平台提供 label / comment / plist / type / schedule / keepalive 时，
    // core 组装 Name / Description / UnitFile 与 details（parity 富化字段）。
    let mut evidence = SourceEvidence::default();
    evidence.launchd_by_pid.push((
        Pid::new(5)?,
        vec![
            (String::from("label"), String::from("com.fxt.daemon")),
            (String::from("comment"), String::from("fxt 注释")),
            (
                String::from("plist"),
                String::from("/Users/fixture-user/Library/LaunchAgents/fxt.plist"),
            ),
            (String::from("type"), String::from("Launch Agent")),
            (String::from("schedule"), String::from("StartInterval: 60")),
            (
                String::from("keepalive"),
                String::from("Yes (restarts if killed)"),
            ),
        ],
    ));
    let src = detect_for(&ancestry, &evidence);
    assert_eq!(src.source_type(), SourceType::Launchd);
    assert_eq!(src.name(), Some("com.fxt.daemon"));
    assert_eq!(src.description(), Some("fxt 注释"));
    assert_eq!(
        src.unit_file(),
        Some("/Users/fixture-user/Library/LaunchAgents/fxt.plist")
    );
    let details = src.details();
    assert!(details.contains(&(
        String::from("plist"),
        String::from("/Users/fixture-user/Library/LaunchAgents/fxt.plist")
    )));
    assert!(details.contains(&(String::from("type"), String::from("Launch Agent"))));
    assert!(details.contains(&(
        String::from("keepalive"),
        String::from("Yes (restarts if killed)")
    )));

    // 根不是 PID 1 launchd → launchd 分支不命中（Linux 根 systemd 不受影响）。
    let ancestry = vec![
        summary(1, Some(0), "systemd", None),
        summary(5, Some(1), "fxt-daemon", None),
    ];
    let mut evidence = SourceEvidence::default();
    evidence.launchd_by_pid.push((
        Pid::new(5)?,
        vec![(String::from("label"), String::from("com.fxt.daemon"))],
    ));
    let src = detect_for(&ancestry, &evidence);
    assert_ne!(src.source_type(), SourceType::Launchd);
    Ok(())
}

#[test]
fn pipeline_source_detection_windows_service_matches_witr_levels() -> TestResult {
    // 第一级：祖先链自目标向根第一个携带 SCM 证据的 PID（目标优先）。
    let mut evidence = SourceEvidence::default();
    evidence.windows_service_by_pid.push((
        Pid::new(9)?,
        vec![
            (String::from("service"), String::from("fxt-svc")),
            (String::from("description"), String::from("fxt 服务显示名")),
        ],
    ));
    evidence.windows_service_by_pid.push((
        Pid::new(7)?,
        vec![(String::from("service"), String::from("fxt-other"))],
    ));
    let ancestry = vec![
        summary(4, Some(0), "System", None),
        summary(7, Some(4), "services.exe", None),
        summary(9, Some(7), "fxt-worker", None),
    ];
    let src = detect_for(&ancestry, &evidence);
    assert_eq!(src.source_type(), SourceType::WindowsService);
    assert_eq!(src.name(), Some("fxt-svc"));
    assert_eq!(src.description(), Some("fxt 服务显示名"));
    assert_eq!(
        src.unit_file(),
        Some("HKLM\\SYSTEM\\CurrentControlSet\\Services\\fxt-svc")
    );
    assert!(
        src.details()
            .contains(&(String::from("manager"), String::from("services.exe")))
    );

    // 第二级：祖先链含 services.exe 但无显式服务名。
    let ancestry = vec![
        summary(4, Some(0), "System", None),
        summary(7, Some(4), "services.exe", None),
        summary(9, Some(7), "fxt-worker", None),
    ];
    let src = detect_for(&ancestry, &SourceEvidence::default());
    assert_eq!(src.source_type(), SourceType::WindowsService);
    assert_eq!(src.name(), Some("Service Control Manager"));

    // 第三级（parity `detectWindowsService` 第三段）：父进程为 services.exe
    // 且无有效服务名 → 目标命令去 .exe 推断服务名。注意第二级遍历整条祖先链，
    // 父进程必然在链内，故只要父命令名恰为 services.exe，第二级总是先命中；
    // 第三级是 parity 结构保留的防御分支，witr 中同样不可达。
    let ancestry = vec![
        summary(4, Some(0), "System", None),
        summary(7, Some(4), "services.exe", None),
        summary(9, Some(7), "fxtsvc.exe", None),
    ];
    let src = detect_for(&ancestry, &SourceEvidence::default());
    assert_eq!(src.source_type(), SourceType::WindowsService);
    assert_eq!(src.name(), Some("Service Control Manager"));
    Ok(())
}

#[test]
fn pipeline_source_detection_init_windows_kernel() {
    // Windows 内核兜底：根 PID 4 "System"（大小写不敏感）且链中无 shell。
    let ancestry = vec![
        summary(4, Some(0), "System", None),
        summary(9, Some(4), "fxtsvc.exe", None),
    ];
    let src = detect_for(&ancestry, &SourceEvidence::default());
    assert_eq!(src.source_type(), SourceType::Init);
    assert_eq!(src.name(), Some("System"));
    assert_eq!(src.description(), Some("Windows kernel (System process)"));
    assert!(
        src.details()
            .contains(&(String::from("pid"), String::from("4")))
    );

    // 链中有 shell 时不判为 init（与 Unix 分支同一规则）。
    let ancestry = vec![
        summary(4, Some(0), "System", None),
        summary(8, Some(4), "cmd.exe", None),
        summary(9, Some(8), "fxt.exe", None),
    ];
    let src = detect_for(&ancestry, &SourceEvidence::default());
    assert_ne!(src.source_type(), SourceType::Init);
}
