//! 外部命令执行端口（容器 CLI、`lsof`、`launchctl` 的唯一通道）。

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::model::error::InspectError;

/// 可用性探测超时（parity/总计划·外部命令边界：500ms）。
pub const PROBE_TIMEOUT: Duration = Duration::from_millis(500);
/// 列表调用超时（3s）。
pub const LIST_TIMEOUT: Duration = Duration::from_secs(3);
/// 详情调用超时（5s）。
pub const DETAIL_TIMEOUT: Duration = Duration::from_secs(5);
/// 单次 stdout 输出上限（8MiB）。
pub const STDOUT_LIMIT_BYTES: usize = 8 * 1024 * 1024;
/// 单次 stderr 输出上限（8MiB）。
pub const STDERR_LIMIT_BYTES: usize = 8 * 1024 * 1024;

/// 一次外部命令调用的规格：程序名 + 独立 argv。
///
/// 不允许任何 shell 参与；调用方不得把多条命令拼接进 `program` 或 `args`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandSpec {
    /// 程序名或绝对路径。
    pub program: String,
    /// 独立参数列表，逐项传递给目标程序。
    pub args: Vec<String>,
}

impl CommandSpec {
    /// 由程序名与参数列表构造调用规格。
    pub fn new<I, S, A>(program: I, args: A) -> Self
    where
        I: Into<String>,
        S: Into<String>,
        A: IntoIterator<Item = S>,
    {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
        }
    }
}

/// 外部命令的执行结果。
///
/// `*_truncated` 为 `true` 表示对应输出流超过上限被截断；实现必须同时追加
/// [`DiagnosticCode::OutputLimitExceeded`](crate::model::diagnostic::DiagnosticCode::OutputLimitExceeded)
/// 类诊断，不得静默截断。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandOutput {
    /// 进程退出码；被信号终止或无法取得时为 `None`。
    pub exit_code: Option<i32>,
    /// 标准输出原始字节（可能不是合法 UTF-8）。
    pub stdout: Vec<u8>,
    /// 标准错误原始字节。
    pub stderr: Vec<u8>,
    /// stdout 是否被截断。
    pub stdout_truncated: bool,
    /// stderr 是否被截断。
    pub stderr_truncated: bool,
}

/// 外部命令执行端口。
///
/// 前置条件：
/// * 不经过 shell，只接受 [`CommandSpec`] 中的程序名与独立 argv；
/// * `timeout` 取 [`PROBE_TIMEOUT`] / [`LIST_TIMEOUT`] / [`DETAIL_TIMEOUT`] 之一或调用方明确值。
///
/// 后置条件：
/// * 超时、无法启动、程序缺失都返回 [`InspectError::ExternalTool`]（细节写入 `detail`），
///   不得 panic，也不得返回半截结果当作成功；
/// * stdout / stderr 分别受 [`STDOUT_LIMIT_BYTES`] / [`STDERR_LIMIT_BYTES`] 约束。
pub trait CommandRunner {
    /// 执行一次外部命令并采集输出。
    fn run(&self, spec: &CommandSpec, timeout: Duration) -> Result<CommandOutput, InspectError>;
}
