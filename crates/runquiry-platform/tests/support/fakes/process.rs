use std::path::PathBuf;

use runquiry_core::{
    CapabilityStatus, HealthStatus, InspectError, Inspection, ProcessDetails,
    ProcessDetailsProvider, ProcessIdentity, ProcessInventory, ProcessSummary,
};

use super::{FakePlatform, captured_at, synthetic_identity};

impl ProcessInventory for FakePlatform {
    fn capability(&self) -> CapabilityStatus {
        self.capability_for()
    }

    fn list(&self) -> Inspection<Vec<ProcessSummary>> {
        let summary = ProcessSummary {
            identity: synthetic_identity(self.baseline.pid(), Some(captured_at())),
            parent_pid: None,
            command: String::from("fxt-daemon"),
            command_line: Some(String::from(
                "fxt-daemon --config /opt/runquiry-fixtures/etc/fxt.conf",
            )),
            user: Some(String::from("fixture-user")),
            health: HealthStatus::Healthy,
            container: None,
            exe_deleted: false,
            capabilities: Vec::new(),
        };
        self.inspect(vec![summary])
    }
}

impl ProcessDetailsProvider for FakePlatform {
    fn capability(&self) -> CapabilityStatus {
        self.capability_for()
    }

    fn details(
        &self,
        identity: &ProcessIdentity,
    ) -> Result<Inspection<ProcessDetails>, InspectError> {
        if !identity.same_process(&self.baseline) {
            return Err(InspectError::NotFound {
                subject: format!("PID {}", identity.pid()),
            });
        }
        if self.scenario == super::super::Scenario::PermissionDenied {
            return Err(InspectError::PermissionDenied {
                subject: String::from("进程详情（合成场景）"),
            });
        }
        Ok(Inspection::complete(ProcessDetails {
            identity: self.baseline.clone(),
            cpu_percent: Some(12.5),
            memory_rss_bytes: Some(20_480),
            memory_percent: Some(0.5),
            working_dir: Some(PathBuf::from("/opt/runquiry-fixtures/var")),
            environment: Vec::new(),
            children: Vec::new(),
            memory: None,
            io: None,
            open_files: Vec::new(),
            fd_count: None,
            fd_limit: None,
        }))
    }
}
