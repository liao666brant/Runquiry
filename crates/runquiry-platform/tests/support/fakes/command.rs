use std::time::Duration;

use runquiry_core::{CommandOutput, CommandRunner, CommandSpec, InspectError};

use super::FakePlatform;

impl CommandRunner for FakePlatform {
    fn run(&self, spec: &CommandSpec, _timeout: Duration) -> Result<CommandOutput, InspectError> {
        let program = spec.program.clone();
        match self.scenario {
            super::super::Scenario::ToolMissing => Err(InspectError::ExternalTool {
                program,
                detail: String::from("程序未安装（合成场景）"),
            }),
            super::super::Scenario::Timeout => Err(InspectError::ExternalTool {
                program,
                detail: String::from("超时（合成场景）"),
            }),
            super::super::Scenario::MalformedOutput => Ok(CommandOutput {
                exit_code: Some(0),
                stdout: vec![0xFF, 0xFE, 0x00],
                stderr: Vec::new(),
                stdout_truncated: false,
                stderr_truncated: false,
            }),
            super::super::Scenario::Normal
            | super::super::Scenario::Empty
            | super::super::Scenario::Partial
            | super::super::Scenario::PermissionDenied => Ok(CommandOutput {
                exit_code: Some(0),
                stdout: b"[]".to_vec(),
                stderr: Vec::new(),
                stdout_truncated: false,
                stderr_truncated: false,
            }),
        }
    }
}
