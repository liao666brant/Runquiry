use runquiry_core::{
    CapabilityStatus, ContainerInventory, ContainerKey, ContainerSummary, DiagnosticCode,
    InspectError, Inspection, Pid, Port,
};

use super::{FakePlatform, captured_at, issue};

const FIXTURE_PORT: Port = match Port::new(8443) {
    Ok(port) => port,
    Err(_) => Port::MIN,
};

fn fixture_container(pid: Pid) -> ContainerSummary {
    ContainerSummary {
        key: ContainerKey {
            runtime: String::from("docker"),
            id: String::from("fxt0001container"),
        },
        name: Some(String::from("fxt-web")),
        image: Some(String::from("registry.example.internal/fxt-web:1.0")),
        status: Some(String::from("Up 2 hours")),
        health: None,
        host_pid: Some(pid),
        started_at: Some(captured_at()),
    }
}

impl ContainerInventory for FakePlatform {
    fn capability(&self) -> CapabilityStatus {
        self.capability_for()
    }

    fn list(&self) -> Inspection<Vec<ContainerSummary>> {
        let container = fixture_container(self.baseline.pid());
        if self.scenario == super::super::Scenario::ToolMissing {
            return Inspection::with_captured_at(
                Some(vec![container]),
                vec![issue(
                    DiagnosticCode::ExternalToolFailed,
                    "运行时 podman CLI 缺失，其容器未计入（合成场景）",
                )],
                captured_at(),
            );
        }
        self.inspect(vec![container])
    }

    fn published_on(&self, port: Port) -> Inspection<Vec<ContainerSummary>> {
        if port == FIXTURE_PORT {
            self.list()
        } else {
            self.inspect(Vec::new())
        }
    }

    fn host_pid(&self, key: &ContainerKey) -> Result<Option<Pid>, InspectError> {
        match key.id.as_str() {
            "fxt0001container" => Ok(Some(self.baseline.pid())),
            _ => Ok(None),
        }
    }
}
