//! 生产 [`CommandRunner`](runquiry_core::CommandRunner) 实现。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use runquiry_core::{
    CancellationToken, CommandOutput, CommandRunner as CommandRunnerPort, CommandSpec, InspectError,
};

use super::output::{collect, spawn_readers};
use super::process::{reap, spawn};

const POLL_INTERVAL: Duration = Duration::from_millis(5);

/// 命令边界的类型化失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandFailure {
    /// 程序缺失或无法启动。
    Spawn {
        /// 程序名或路径。
        program: String,
        /// 失败细节（不含参数值与环境变量）。
        detail: String,
    },
    /// 超时；子进程组已被终止并回收。
    Timeout {
        /// 程序名或路径。
        program: String,
        /// 超时时长。
        timeout: Duration,
    },
    /// stdout 或 stderr 超过上限；子进程组已被终止并回收。
    OutputLimit {
        /// 程序名或路径。
        program: String,
        /// stdout 是否观测到超限。
        stdout: bool,
        /// stderr 是否观测到超限。
        stderr: bool,
    },
    /// 调用方取消；已启动的子进程组已被终止并回收。
    Cancelled {
        /// 程序名或路径。
        program: String,
    },
}

impl CommandFailure {
    /// 转换为端口契约要求的 [`InspectError::ExternalTool`]。
    pub fn into_inspect_error(self) -> InspectError {
        match self {
            Self::Spawn { program, detail } => InspectError::ExternalTool {
                program,
                detail: format!("无法启动：{detail}"),
            },
            Self::Timeout { program, timeout } => InspectError::ExternalTool {
                program,
                detail: format!("超时（{}ms），子进程已终止并回收", timeout.as_millis()),
            },
            Self::OutputLimit {
                program,
                stdout,
                stderr,
            } => InspectError::ExternalTool {
                program,
                detail: format!(
                    "命令输出超过上限（stdout={stdout}, stderr={stderr}），子进程已终止并回收"
                ),
            },
            Self::Cancelled { program } => InspectError::ExternalTool {
                program,
                detail: String::from("命令已取消，子进程已终止并回收"),
            },
        }
    }
}

/// `std::process` 命令执行器。
#[derive(Debug, Clone, Copy, Default)]
pub struct StdCommandRunner;

impl StdCommandRunner {
    /// 执行命令并保留分类失败。
    pub fn run_classified(
        &self,
        spec: &CommandSpec,
        timeout: Duration,
    ) -> Result<CommandOutput, CommandFailure> {
        self.run_classified_with_cancellation(spec, timeout, &CancellationToken::new())
    }

    /// 执行可取消命令并保留分类失败。
    pub fn run_classified_with_cancellation(
        &self,
        spec: &CommandSpec,
        timeout: Duration,
        cancellation: &CancellationToken,
    ) -> Result<CommandOutput, CommandFailure> {
        Self::execute(spec, timeout, cancellation, false)
    }

    /// sudo 运行时尝试以原始普通用户执行。
    pub fn run_classified_as_original_user(
        &self,
        spec: &CommandSpec,
        timeout: Duration,
    ) -> Result<CommandOutput, CommandFailure> {
        Self::execute(spec, timeout, &CancellationToken::new(), true)
    }

    fn execute(
        spec: &CommandSpec,
        timeout: Duration,
        cancellation: &CancellationToken,
        original_user: bool,
    ) -> Result<CommandOutput, CommandFailure> {
        if cancellation.is_cancelled() {
            return Err(CommandFailure::Cancelled {
                program: spec.program.clone(),
            });
        }
        let mut child = spawn(spec, original_user)?;
        let stdout_hit = Arc::new(AtomicBool::new(false));
        let stderr_hit = Arc::new(AtomicBool::new(false));
        let readers =
            match spawn_readers(&mut child, Arc::clone(&stdout_hit), Arc::clone(&stderr_hit)) {
                Ok(readers) => readers,
                Err(error) => {
                    reap(&mut child);
                    return Err(output_failure(spec, "初始化", error));
                }
            };

        let deadline = Instant::now() + timeout;
        let stop = loop {
            match child.try_wait() {
                Ok(Some(status)) => break StopReason::Exited(status.code()),
                Ok(None) => {}
                Err(error) => {
                    reap(&mut child);
                    return Err(CommandFailure::Spawn {
                        program: spec.program.clone(),
                        detail: format!("等待进程状态失败：{error}"),
                    });
                }
            }
            if stdout_hit.load(Ordering::SeqCst) || stderr_hit.load(Ordering::SeqCst) {
                break StopReason::OutputLimit;
            }
            if cancellation.is_cancelled() {
                break StopReason::Cancelled;
            }
            if Instant::now() >= deadline {
                break StopReason::Timeout;
            }
            thread::sleep(POLL_INTERVAL);
        };
        if !matches!(stop, StopReason::Exited(_)) {
            reap(&mut child);
        }
        let stdout = collect(readers.stdout);
        let stderr = collect(readers.stderr);
        let limits = OutputLimits {
            stdout: stdout_hit.load(Ordering::SeqCst),
            stderr: stderr_hit.load(Ordering::SeqCst),
        };
        if limits.stdout || limits.stderr || stop == StopReason::OutputLimit {
            return Err(CommandFailure::OutputLimit {
                program: spec.program.clone(),
                stdout: limits.stdout,
                stderr: limits.stderr,
            });
        }
        let exit_code = match stop {
            StopReason::Cancelled => {
                return Err(CommandFailure::Cancelled {
                    program: spec.program.clone(),
                });
            }
            StopReason::Timeout => {
                return Err(CommandFailure::Timeout {
                    program: spec.program.clone(),
                    timeout,
                });
            }
            StopReason::Exited(_) if cancellation.is_cancelled() => {
                return Err(CommandFailure::Cancelled {
                    program: spec.program.clone(),
                });
            }
            StopReason::Exited(exit_code) => exit_code,
            StopReason::OutputLimit => {
                return Err(CommandFailure::OutputLimit {
                    program: spec.program.clone(),
                    stdout: limits.stdout,
                    stderr: limits.stderr,
                });
            }
        };
        let stdout = stdout.map_err(|error| output_failure(spec, "stdout", error))?;
        let stderr = stderr.map_err(|error| output_failure(spec, "stderr", error))?;
        Ok(CommandOutput {
            exit_code,
            stdout,
            stderr,
            stdout_truncated: false,
            stderr_truncated: false,
        })
    }
}

fn output_failure(
    spec: &CommandSpec,
    stream: &str,
    error: impl std::fmt::Display,
) -> CommandFailure {
    CommandFailure::Spawn {
        program: spec.program.clone(),
        detail: format!("收取 {stream} 失败：{error}"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StopReason {
    Exited(Option<i32>),
    Timeout,
    OutputLimit,
    Cancelled,
}

#[derive(Debug, Clone, Copy)]
struct OutputLimits {
    stdout: bool,
    stderr: bool,
}

impl CommandRunnerPort for StdCommandRunner {
    fn run(&self, spec: &CommandSpec, timeout: Duration) -> Result<CommandOutput, InspectError> {
        self.run_classified(spec, timeout)
            .map_err(CommandFailure::into_inspect_error)
    }

    fn run_with_cancellation(
        &self,
        spec: &CommandSpec,
        timeout: Duration,
        cancellation: &CancellationToken,
    ) -> Result<CommandOutput, InspectError> {
        self.run_classified_with_cancellation(spec, timeout, cancellation)
            .map_err(CommandFailure::into_inspect_error)
    }
}
