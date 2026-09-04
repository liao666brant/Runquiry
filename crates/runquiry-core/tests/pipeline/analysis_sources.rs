//! 分析后的 systemd 与容器来源契约。

use runquiry_core::{
    AnalysisPorts, Inspection, Pid, ProcessDetails, SourceEvidence, SourceType, analyze,
};

use crate::support::collectors::{
    FakeContainers, FakeDetails, FakeEvidence, FakeInventory, FakeNetwork, captured_at, summary,
};

use super::TestResult;
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
