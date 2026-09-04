//! 分析采集器组合与退化契约。

use std::time::Duration;

use runquiry_core::{
    AnalysisPorts, DiagnosticCode, InspectError, Inspection, Pid, ProcessDetails, ProcessIdentity,
    SourceEvidence, SourceType, analyze,
};

use crate::support::collectors::{
    FakeContainers, FakeDetails, FakeEvidence, FakeInventory, FakeNetwork, FakeProcessLocks,
    captured_at, summary,
};

use super::TestResult;
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
    let details = FakeDetails::partial(
        &target.identity,
        details,
        vec![runquiry_core::DiagnosticIssue::new(
            DiagnosticCode::ParseFailed,
            String::from("合成详情字段损坏"),
        )],
    );
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
    assert!(analysis.details.is_some(), "详情部分成功的数据必须保留");
    assert!(
        inspection
            .issues
            .iter()
            .any(|issue| issue.code() == DiagnosticCode::ParseFailed),
        "详情字段诊断必须合并到分析结果"
    );
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
            outcome: outcome.map(Inspection::complete),
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
