//! 假平台后端消费测试（A5 任务 3）：七个端口 trait 在八类场景下的行为、
//! 确定性时钟 / 固定 generation 与失败注入入口。
//!
//! 不依赖真实进程、真实容器 CLI 或宿主机状态；快照时刻与 generation
//! 均为合成固定值，与 `tests/fixtures/` 同值约定。

mod support;

use std::path::Path;
use std::time::{Duration, SystemTime};

use runquiry_core::{
    CapabilityStatus, CommandRunner, CommandSpec, ContainerInventory, ContainerKey, DETAIL_TIMEOUT,
    DiagnosticCode, FileInventory, InspectError, NetworkInventory as _, Pid, ProcessAction,
    ProcessController, ProcessDetailsProvider, ProcessIdentity, ProcessInventory,
};
use support::fakes::FakePlatform;
use support::{CAPTURED_AT_MS, FXT_PID, FixedClock, Generation, Scenario};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn expected_captured_at() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_millis(CAPTURED_AT_MS)
}

fn fxt_identity(
    start_time: Option<SystemTime>,
) -> Result<ProcessIdentity, Box<dyn std::error::Error>> {
    Ok(ProcessIdentity::new(
        Pid::new(FXT_PID)?,
        start_time,
        Some(std::path::PathBuf::from(
            "/opt/runquiry-fixtures/bin/fxt-daemon",
        )),
    ))
}

#[test]
fn normal_scenario_provides_data_on_all_ports() -> TestResult {
    let platform = FakePlatform::new(Scenario::Normal)?;

    let listed = platform.process_list();
    let entries = listed
        .data
        .as_deref()
        .ok_or_else(|| String::from("normal 场景必须有进程数据"))?;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].command, "fxt-daemon");
    assert_eq!(listed.captured_at, expected_captured_at());
    assert!(!listed.has_issues());

    let identity = fxt_identity(Some(expected_captured_at()))?;
    let details = platform
        .details(&identity)?
        .data
        .ok_or_else(|| String::from("normal 场景必须有进程详情"))?;
    assert!(details.identity.same_process(&identity));
    assert_eq!(details.memory_rss_bytes, Some(20_480));

    assert_eq!(platform.open_ports().data.as_ref().map(Vec::len), Some(1));
    let pid = Pid::new(FXT_PID)?;
    assert_eq!(
        platform.sockets_of(pid).data.as_ref().map(Vec::len),
        Some(1)
    );

    let key = ContainerKey {
        runtime: String::from("docker"),
        id: String::from("fxt0001container"),
    };
    assert_eq!(
        platform.list_containers().data.as_ref().map(Vec::len),
        Some(1)
    );
    assert_eq!(platform.container_host_pid(&key)?, Some(pid));
    assert_eq!(
        platform
            .lock_holders(Path::new("/opt/runquiry-fixtures/var/fxt-daemon.lock"))
            .data
            .as_ref()
            .map(Vec::len),
        Some(1)
    );

    let output = platform.run(&CommandSpec::new("fxt-runtime", ["ps"]), DETAIL_TIMEOUT)?;
    assert_eq!(output.exit_code, Some(0));
    assert_eq!(output.stdout, b"[]".to_vec());
    Ok(())
}

#[test]
fn empty_scenario_yields_complete_empty_collections() -> TestResult {
    let platform = FakePlatform::new(Scenario::Empty)?;

    let listed = platform.process_list();
    assert_eq!(
        listed.data.as_deref().map(<[_]>::len),
        Some(0),
        "空结果不是失败"
    );
    assert!(!listed.has_issues());
    assert_eq!(platform.open_ports().data, Some(Vec::new()));
    Ok(())
}

#[test]
fn partial_scenario_keeps_data_alongside_issue() -> TestResult {
    let platform = FakePlatform::new(Scenario::Partial)?;

    let listed = platform.process_list();
    assert_eq!(
        listed.data.as_deref().map(<[_]>::len),
        Some(1),
        "不得丢弃其余条目"
    );
    assert_eq!(listed.issues.len(), 1);
    assert_eq!(listed.issues[0].code().code(), "permission_denied");
    Ok(())
}

#[test]
fn permission_scenario_fails_without_data() -> TestResult {
    let platform = FakePlatform::new(Scenario::PermissionDenied)?;

    let listed = platform.process_list();
    assert!(listed.is_empty(), "权限整体失败时不得有数据");
    assert_eq!(listed.issues[0].code().code(), "permission_denied");

    let identity = fxt_identity(Some(expected_captured_at()))?;
    let err = platform
        .details(&identity)
        .err()
        .ok_or_else(|| String::from("权限失败时详情必须报错"))?;
    assert_eq!(err.code(), "permission_denied");
    Ok(())
}

#[test]
fn tool_missing_scenario_reports_external_tool_and_unavailable_capability() -> TestResult {
    let platform = FakePlatform::new(Scenario::ToolMissing)?;

    // 容器：docker 可用 + podman 缺失 = 部分成功（单运行时失败不整体失败）。
    let containers = platform.list_containers();
    assert_eq!(containers.data.as_deref().map(<[_]>::len), Some(1));
    assert_eq!(containers.issues[0].code().code(), "external_tool_failed");
    assert_eq!(
        platform.containers_capability(),
        CapabilityStatus::Unavailable(String::from("容器运行时 CLI 未安装（合成场景）")),
        "运行时缺失是能力状态，不是空集合"
    );

    // 外部命令：工具缺失必须返回 ExternalTool，不得返回半截结果。
    let err = platform
        .run(&CommandSpec::new("fxt-runtime", ["ps"]), DETAIL_TIMEOUT)
        .err()
        .ok_or_else(|| String::from("工具缺失时 run 必须报错"))?;
    assert_eq!(err.code(), "external_tool");
    Ok(())
}

#[test]
fn timeout_scenario_reports_timeout_issue_and_runner_error() -> TestResult {
    let platform = FakePlatform::new(Scenario::Timeout)?;

    let listed = platform.process_list();
    assert!(listed.is_empty());
    assert_eq!(listed.issues[0].code().code(), "timeout");

    let err = platform
        .run(&CommandSpec::new("fxt-runtime", ["ps"]), DETAIL_TIMEOUT)
        .err()
        .ok_or_else(|| String::from("超时时 run 必须报错"))?;
    assert_eq!(err.code(), "external_tool");
    Ok(())
}

#[test]
fn malformed_runner_output_fails_consumer_parse() -> TestResult {
    let platform = FakePlatform::new(Scenario::MalformedOutput)?;

    // 输出损坏时 run 本身成功（命令执行了），但输出不可解析：
    // 消费方必须以 ParseFailed 诊断表达，不得当作空结果。
    let output = platform.run(&CommandSpec::new("fxt-runtime", ["ps"]), DETAIL_TIMEOUT)?;
    assert_eq!(output.exit_code, Some(0));
    let parse: Result<String, _> =
        String::from_utf8(output.stdout).map_err(|_| DiagnosticCode::ParseFailed);
    assert!(parse.is_err(), "损坏输出必须被判为 ParseFailed");
    Ok(())
}

#[test]
fn controller_rejects_reused_pid_and_unverifiable_identity_without_side_effect() -> TestResult {
    // 同 PID 不同 start_time（PID 复用）→ ProcessChanged，动作数 0。
    let reused = FakePlatform::new(Scenario::Normal)?.with_current(fxt_identity(Some(
        expected_captured_at() + Duration::from_hours(1),
    ))?);
    let expected = fxt_identity(Some(expected_captured_at()))?;
    let result = reused.execute(&expected, ProcessAction::Terminate);
    assert!(
        matches!(result, Err(InspectError::ProcessChanged { .. })),
        "PID 复用必须被拒绝"
    );
    assert_eq!(reused.executed_count(), 0, "动作数必须保持 0");

    // current start_time 为 None（身份不可验证）→ 拒绝，动作数 0。
    let unverifiable = FakePlatform::new(Scenario::Normal)?.with_current(fxt_identity(None)?);
    let result = unverifiable.execute(&expected, ProcessAction::Kill);
    assert!(
        matches!(result, Err(InspectError::ProcessChanged { .. })),
        "身份不可验证时必须拒绝执行"
    );
    assert_eq!(unverifiable.executed_count(), 0, "动作数必须保持 0");

    // 身份一致 → 执行，动作数 1。
    let matching = FakePlatform::new(Scenario::Normal)?;
    matching.execute(&expected, ProcessAction::Pause)?;
    assert_eq!(matching.executed_count(), 1, "身份一致时恰好执行一次");
    Ok(())
}

#[test]
fn snapshots_use_deterministic_clock_and_fixed_generation() -> TestResult {
    let platform = FakePlatform::new(Scenario::Normal)?;

    assert_eq!(platform.process_list().captured_at, expected_captured_at());
    assert_eq!(platform.generation(), Generation::FIXTURE.get());

    // 时钟只在测试显式推进时变化，不读墙钟。
    let mut clock = FixedClock::at_epoch_ms(CAPTURED_AT_MS)?;
    clock.advance(Duration::from_secs(3));
    assert_eq!(clock.now(), expected_captured_at() + Duration::from_secs(3));
    Ok(())
}

/// 覆盖 `ProcessInventory::capability` 直读路径（避免 `dead_code` 之外，
/// 主要验证能力状态可被调用方明确区分）。
#[test]
fn capability_status_is_distinguishable_per_scenario() -> TestResult {
    let normal = FakePlatform::new(Scenario::Normal)?;
    assert_eq!(
        ProcessInventory::capability(&normal),
        CapabilityStatus::Supported
    );
    let tool_missing = FakePlatform::new(Scenario::ToolMissing)?;
    assert!(matches!(
        ProcessInventory::capability(&tool_missing),
        CapabilityStatus::Unavailable(_)
    ));
    Ok(())
}

// 便捷方法：把 trait 方法调用收敛到一处，避免测试里重复的完全限定语法。
impl FakePlatform {
    /// 进程基线（`ProcessInventory::list` 的显式消歧调用）。
    fn process_list(&self) -> runquiry_core::Inspection<Vec<runquiry_core::ProcessSummary>> {
        ProcessInventory::list(self)
    }

    fn list_containers(&self) -> runquiry_core::Inspection<Vec<runquiry_core::ContainerSummary>> {
        ContainerInventory::list(self)
    }

    fn containers_capability(&self) -> CapabilityStatus {
        ContainerInventory::capability(self)
    }

    fn container_host_pid(&self, key: &ContainerKey) -> Result<Option<Pid>, InspectError> {
        ContainerInventory::host_pid(self, key)
    }

    fn lock_holders(
        &self,
        path: &Path,
    ) -> runquiry_core::Inspection<Vec<runquiry_core::FileInventoryEntry>> {
        FileInventory::holders(self, path)
    }
}
