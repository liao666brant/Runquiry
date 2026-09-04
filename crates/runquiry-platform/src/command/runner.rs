//! 生产 [`CommandRunner`](runquiry_core::port::command::CommandRunner) 实现。

use std::io::Read;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::thread;
use std::time::{Duration, Instant};

use runquiry_core::{CommandOutput, CommandRunner as CommandRunnerPort, CommandSpec, InspectError};

/// 子进程输出读取的轮询间隔与回收宽限：超时/超限判定到 kill 之间不引入长延迟。
const POLL_INTERVAL: Duration = Duration::from_millis(5);
/// kill 并 wait 之后等待输出线程退出的宽限（正常情况下瞬间完成）。
const DRAIN_GRACE: Duration = Duration::from_secs(1);
/// 读取块大小（64 KiB，与 Linux 管道缓冲同量级）。
const READ_CHUNK: usize = 64 * 1024;

/// 命令执行失败的分类结果：供容器适配器把失败映射为对应诊断类别。
///
/// 分类只含「命令边界」失败；非零退出码不是失败（调用方按 `exit_code` 自行判定）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandFailure {
    /// 程序缺失或无法启动。
    Spawn {
        /// 程序名或路径。
        program: String,
        /// 失败细节（不含参数值与环境变量）。
        detail: String,
    },
    /// 超时；子进程已被终止并回收。
    Timeout {
        /// 程序名或路径。
        program: String,
        /// 超时时长。
        timeout: Duration,
    },
}

impl CommandFailure {
    /// 转换为端口契约要求的类型化 [`InspectError::ExternalTool`]。
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
        }
    }
}

/// [`CommandRunner`](runquiry_core::port::command::CommandRunner) 的 `std::process` 实现。
///
/// 约束（parity/总计划·外部命令边界）：
/// * 只接受 [`CommandSpec`] 的程序名 + 独立 argv，绝不经过 shell；
/// * stdout / stderr 各由独立线程并发读取，分别受
///   [`STDOUT_LIMIT_BYTES`](runquiry_core::port::command::STDOUT_LIMIT_BYTES) /
///   [`STDERR_LIMIT_BYTES`](runquiry_core::port::command::STDERR_LIMIT_BYTES) 限幅；
/// * 超时或任一流超限即 kill 子进程并 `wait` 回收，不留僵尸或遗留进程。
#[derive(Debug, Clone, Copy, Default)]
pub struct StdCommandRunner;

/// ETXTBSY 的原始错误码（`EBUSY` 在 execve 语境下的映射）。
const ETXTBSY: i32 = 26;

impl StdCommandRunner {
    /// 执行命令并分类失败（[`CommandFailure`]），供需要区分超时/缺失的调用方使用。
    ///
    /// 非零退出码不是本层错误：结果原样返回，由调用方判定。
    pub fn run_classified(
        &self,
        spec: &CommandSpec,
        timeout: Duration,
    ) -> Result<CommandOutput, CommandFailure> {
        let mut child = spawn(spec)?;
        let stdout_limit_hit = Arc::new(AtomicBool::new(false));
        let stderr_limit_hit = Arc::new(AtomicBool::new(false));

        let stdout_pipe = child.stdout.take();
        let stderr_pipe = child.stderr.take();
        let stdout_rx = stdout_pipe.map(|pipe| {
            spawn_reader(
                "runquiry-cmd-stdout",
                pipe,
                runquiry_core::STDOUT_LIMIT_BYTES,
                stdout_limit_hit.clone(),
            )
        });
        let stderr_rx = stderr_pipe.map(|pipe| {
            spawn_reader(
                "runquiry-cmd-stderr",
                pipe,
                runquiry_core::STDERR_LIMIT_BYTES,
                stderr_limit_hit.clone(),
            )
        });

        let deadline = Instant::now() + timeout;
        let mut timed_out = false;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {}
                Err(err) => {
                    reap(&mut child);
                    return Err(CommandFailure::Spawn {
                        program: spec.program.clone(),
                        detail: format!("等待进程状态失败：{err}"),
                    });
                }
            }
            if stdout_limit_hit.load(Ordering::Relaxed) || stderr_limit_hit.load(Ordering::Relaxed)
            {
                // 任一流超限即终止子进程；读取线程继续排空到 EOF 以保留已到数据。
                reap(&mut child);
                break;
            }
            if Instant::now() >= deadline {
                timed_out = true;
                reap(&mut child);
                break;
            }
            thread::sleep(POLL_INTERVAL);
        }

        let exit_code = match child.wait() {
            Ok(status) => status.code(),
            Err(err) => {
                return Err(CommandFailure::Spawn {
                    program: spec.program.clone(),
                    detail: format!("回收子进程失败：{err}"),
                });
            }
        };
        if timed_out {
            return Err(CommandFailure::Timeout {
                program: spec.program.clone(),
                timeout,
            });
        }

        let stdout = collect(stdout_rx);
        let stderr = collect(stderr_rx);
        Ok(CommandOutput {
            exit_code,
            stdout,
            stderr,
            stdout_truncated: stdout_limit_hit.load(Ordering::Relaxed),
            stderr_truncated: stderr_limit_hit.load(Ordering::Relaxed),
        })
    }
}

/// 独立函数：不依赖实例状态（ZST）。
fn spawn(spec: &CommandSpec) -> Result<Child, CommandFailure> {
    // WSL2 等内核在「多线程并发写入 + 立即 execve」下会瞬态返回 ETXTBSY
    // （实测 ~5%，与文件内容无关）；短退避重试即恢复，正常路径零开销。
    let mut last_err: Option<std::io::Error> = None;
    let mut builder = Command::new(&spec.program);
    builder
        .args(&spec.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // 独立进程组：超时/超限时可对整组 SIGKILL，子进程派生的孙进程一并终止。
    #[cfg(unix)]
    builder.process_group(0);
    for attempt in 0..=5_u32 {
        match builder.spawn() {
            Ok(child) => return Ok(child),
            Err(err) if err.raw_os_error() == Some(ETXTBSY) && attempt < 5 => {
                thread::sleep(Duration::from_millis(5 * u64::from(attempt + 1)));
                last_err = Some(err);
            }
            Err(err) => {
                return Err(CommandFailure::Spawn {
                    program: spec.program.clone(),
                    detail: err.to_string(),
                });
            }
        }
    }
    let Some(err) = last_err else {
        return Err(CommandFailure::Spawn {
            program: spec.program.clone(),
            detail: String::from("子进程启动重试耗尽"),
        });
    };
    Err(CommandFailure::Spawn {
        program: spec.program.clone(),
        detail: err.to_string(),
    })
}

impl CommandRunnerPort for StdCommandRunner {
    fn run(&self, spec: &CommandSpec, timeout: Duration) -> Result<CommandOutput, InspectError> {
        self.run_classified(spec, timeout)
            .map_err(CommandFailure::into_inspect_error)
    }
}

/// 启动一个输出读取线程：读到 EOF 或超过 `limit` 为止。
///
/// 超过上限时截断到 `limit` 字节、置位 `limit_hit` 并**继续排空**（丢弃多余
/// 字节）直到 EOF——保证另一条流也有机会完整越过上限，主线程负责 kill。
/// 线程创建失败时直接丢弃发送端，调用方经 `recv_timeout` 得到空数据兜底。
fn spawn_reader<R: Read + Send + 'static>(
    name: &str,
    mut stream: R,
    limit: usize,
    limit_hit: Arc<AtomicBool>,
) -> Receiver<(Vec<u8>, bool)> {
    let (tx, rx) = channel();
    let _ = thread::Builder::new()
        .name(name.to_string())
        .spawn(move || {
            let mut collected = Vec::new();
            let mut truncated = false;
            let mut chunk = vec![0_u8; READ_CHUNK];
            loop {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        collected.extend_from_slice(&chunk[..n]);
                        if collected.len() > limit {
                            collected.truncate(limit);
                            truncated = true;
                            limit_hit.store(true, Ordering::Relaxed);
                        }
                    }
                }
            }
            let _ = tx.send((collected, truncated));
        });
    rx
}

/// 从读取线程收取结果；超宽限未结束时以空数据兜底（子进程的孙进程持有管道
/// 写端的极端场景），不阻塞调用方。
fn collect(rx: Option<Receiver<(Vec<u8>, bool)>>) -> Vec<u8> {
    rx.map_or_else(Vec::new, |rx| match rx.recv_timeout(DRAIN_GRACE) {
        Ok((data, _)) => data,
        Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => Vec::new(),
    })
}

/// 终止子进程并等待回收：先对整个进程组发 SIGKILL（终止子进程派生的孙进程，
/// 如假 CLI 脚本内的 `sleep`），再 kill 直接子进程兜底，最后 wait 回收；
/// 已退出的子进程 kill 失败属预期，忽略即可。
fn reap(child: &mut Child) {
    kill_process_group(child.id());
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(unix)]
fn kill_process_group(process_id: u32) {
    let Ok(pgid) = i32::try_from(process_id) else {
        return;
    };
    // SAFETY: kill(2) 仅向进程组 `-pgid` 发送信号，不解引用任何内存；pgid
    // 由 spawn 时的 `process_group(0)` 设为子进程 PID，属本 runner 创建的
    // 进程组，不存在误伤无关进程组的路径。
    unsafe {
        libc::kill(-pgid, libc::SIGKILL);
    }
}

#[cfg(not(unix))]
fn kill_process_group(_pid: u32) {}
