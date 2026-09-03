//! 七个同步平台端口可被最小假实现消费的契约测试（A3 验收 8）。
//!
//! 本文件不实现真实平台后端，仅证明接口可由 runquiry-platform 实现，
//! 且平台能力通过 [`CapabilityStatus`] 显式表达而非魔法值。

use std::path::{Path, PathBuf};
use std::time::Duration;

use runquiry_core::{
    CapabilityStatus, CommandOutput, CommandRunner, CommandSpec, ContainerInventory, ContainerKey,
    ContainerSummary, DETAIL_TIMEOUT, DiagnosticCode, DiagnosticIssue, FileInventory,
    FileLockEntry, InspectError, Inspection, LIST_TIMEOUT, LockMode, LockType, NetworkInventory,
    OpenPortEntry, Pid, Port, ProcessAction, ProcessController, ProcessDetails,
    ProcessDetailsProvider, ProcessIdentity, ProcessInventory, ProcessSummary, Protocol,
    SocketEntry,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// 进程基线假实现：列表数据由测试函数预构建。
struct FakeProcessInventory {
    capability: CapabilityStatus,
    entries: Vec<ProcessSummary>,
}

impl ProcessInventory for FakeProcessInventory {
    fn capability(&self) -> CapabilityStatus {
        self.capability.clone()
    }

    fn list(&self) -> Inspection<Vec<ProcessSummary>> {
        Inspection::complete(self.entries.clone())
    }
}

/// 进程详情假实现。
struct FakeProcessDetails;

impl ProcessDetailsProvider for FakeProcessDetails {
    fn capability(&self) -> CapabilityStatus {
        CapabilityStatus::Supported
    }

    fn details(&self, identity: &ProcessIdentity) -> Result<ProcessDetails, InspectError> {
        Ok(ProcessDetails {
            identity: identity.clone(),
            cpu_percent: Some(1.5),
            memory_rss_bytes: Some(2048),
            memory_percent: Some(0.1),
            working_dir: Some(PathBuf::from("/srv/app")),
            environment: Vec::new(),
            children: Vec::new(),
        })
    }
}

/// 网络假实现。
struct FakeNetwork {
    port: Port,
}

impl NetworkInventory for FakeNetwork {
    fn capability(&self) -> CapabilityStatus {
        CapabilityStatus::Supported
    }

    fn open_ports(&self) -> Inspection<Vec<OpenPortEntry>> {
        Inspection::complete(vec![OpenPortEntry {
            pid: None,
            port: self.port,
            address: String::from("0.0.0.0"),
            protocol: Protocol::Tcp,
            state: String::from("LISTEN"),
        }])
    }

    fn sockets_of(&self, pid: Pid) -> Inspection<Vec<SocketEntry>> {
        Inspection::complete(vec![SocketEntry {
            inode: Some(1),
            port: Some(self.port),
            address: String::from("127.0.0.1"),
            remote_addr: Some(String::from("10.0.0.8")),
            state: String::from("ESTAB"),
            protocol: Protocol::Tcp,
            owner_pid: Some(pid),
        }])
    }
}

/// 容器假实现：单个运行时失败仅产生 issue，不阻断其他运行时。
struct FakeContainers {
    key: ContainerKey,
    host_pid: Option<Pid>,
}

impl ContainerInventory for FakeContainers {
    fn capability(&self) -> CapabilityStatus {
        CapabilityStatus::Partial(String::from("仅探测到 docker"))
    }

    fn list(&self) -> Inspection<Vec<ContainerSummary>> {
        Inspection::partial(
            vec![ContainerSummary {
                key: self.key.clone(),
                name: Some(String::from("web")),
                image: Some(String::from("nginx:1.27")),
                status: Some(String::from("Up 2 hours")),
                health: None,
                host_pid: self.host_pid,
                started_at: None,
            }],
            vec![DiagnosticIssue::new(
                DiagnosticCode::ExternalToolFailed,
                String::from("podman 不可用"),
            )],
        )
    }

    fn host_pid(&self, key: &ContainerKey) -> Result<Option<Pid>, InspectError> {
        if key == &self.key {
            Ok(self.host_pid)
        } else {
            Ok(None)
        }
    }
}

/// 文件假实现。
struct FakeFiles {
    pid: Pid,
}

impl FileInventory for FakeFiles {
    fn capability(&self) -> CapabilityStatus {
        CapabilityStatus::Supported
    }

    fn holders(&self, path: &Path) -> Inspection<Vec<FileLockEntry>> {
        Inspection::complete(vec![FileLockEntry {
            pid: self.pid,
            process: String::from("nginx"),
            path: path.to_path_buf(),
            lock_type: LockType::Flock,
            mode: LockMode::Write,
        }])
    }
}

/// 进程控制假实现：身份缺失 `start_time` 视为不可验证，直接拒绝。
struct FakeController;

impl ProcessController for FakeController {
    fn capability(&self) -> CapabilityStatus {
        CapabilityStatus::Supported
    }

    fn execute(
        &self,
        identity: &ProcessIdentity,
        action: ProcessAction,
    ) -> Result<(), InspectError> {
        match identity.start_time() {
            Some(_) => {
                let _ = action;
                Ok(())
            }
            None => Err(InspectError::ProcessChanged {
                identity: identity.clone(),
            }),
        }
    }
}

/// 外部命令假实现。
struct FakeRunner;

impl CommandRunner for FakeRunner {
    fn run(&self, spec: &CommandSpec, timeout: Duration) -> Result<CommandOutput, InspectError> {
        assert_eq!(spec.program, "docker");
        assert_eq!(timeout, DETAIL_TIMEOUT);
        Ok(CommandOutput {
            exit_code: Some(0),
            stdout: b"[]".to_vec(),
            stderr: Vec::new(),
            stdout_truncated: false,
            stderr_truncated: false,
        })
    }
}

#[test]
fn all_seven_ports_are_implementable_and_callable() -> TestResult {
    let pid = Pid::new(12)?;
    let host_pid = Pid::new(99)?;
    let port = Port::new(8080)?;
    let started = std::time::SystemTime::UNIX_EPOCH;
    let identity = ProcessIdentity::new(pid, Some(started), None);
    let key = ContainerKey {
        runtime: String::from("docker"),
        id: String::from("abc"),
    };

    let processes = FakeProcessInventory {
        capability: CapabilityStatus::Supported,
        entries: vec![ProcessSummary {
            identity: ProcessIdentity::new(pid, Some(started), None),
            parent_pid: None,
            command: String::from("nginx"),
            command_line: Some(String::from("nginx -g daemon off;")),
            user: Some(String::from("www-data")),
        }],
    };
    assert_eq!(processes.list().data.as_ref().map(Vec::len), Some(1));
    assert_eq!(processes.capability(), CapabilityStatus::Supported);

    let details = processes_details(&FakeProcessDetails, &identity)?;
    assert!(
        details.identity.same_process(&identity),
        "详情快照必须对应同一可验证身份"
    );
    assert_eq!(details.memory_rss_bytes, Some(2048));

    let network = FakeNetwork { port };
    assert_eq!(network.open_ports().data.as_ref().map(Vec::len), Some(1));
    assert_eq!(network.sockets_of(pid).data.as_ref().map(Vec::len), Some(1));

    let containers = FakeContainers {
        key: key.clone(),
        host_pid: Some(host_pid),
    };
    let listed = containers.list();
    assert_eq!(listed.data.as_ref().map(Vec::len), Some(1));
    assert_eq!(listed.issues.len(), 1, "单运行时失败仅产生 issue");
    assert_eq!(containers.host_pid(&key)?, Some(host_pid));

    let locks = FakeFiles { pid }.holders(Path::new("/var/log/app.log"));
    assert_eq!(locks.data.as_ref().map(Vec::len), Some(1));
    assert_eq!(
        locks
            .data
            .as_ref()
            .and_then(|l| l.first())
            .map(|e| e.lock_type),
        Some(LockType::Flock)
    );

    FakeController.execute(&identity, ProcessAction::Terminate)?;
    let changed =
        FakeController.execute(&ProcessIdentity::new(pid, None, None), ProcessAction::Kill);
    assert!(matches!(changed, Err(InspectError::ProcessChanged { .. })));

    let output = FakeRunner.run(
        &CommandSpec::new("docker", ["ps", "--format", "json"]),
        DETAIL_TIMEOUT,
    )?;
    assert_eq!(output.exit_code, Some(0));
    assert_eq!(LIST_TIMEOUT, Duration::from_secs(3));
    Ok(())
}

fn processes_details(
    provider: &FakeProcessDetails,
    identity: &ProcessIdentity,
) -> Result<ProcessDetails, InspectError> {
    ProcessDetailsProvider::details(provider, identity)
}
