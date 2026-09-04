//! 七个平台端口的假后端：按 [`Scenario`] 注入失败，构成失败注入入口。
//!
//! 实现按端口职责拆分到同名子模块；本模块仅保留共享状态和构造入口。

#![allow(unreachable_pub)]
#![allow(dead_code)]

mod command;
mod container;
mod controller;
mod files;
mod network;
mod process;

use std::cell::RefCell;
use std::time::{Duration, SystemTime};

use runquiry_core::{
    CapabilityStatus, DiagnosticCode, DiagnosticIssue, Inspection, Pid, ProcessAction,
    ProcessIdentity,
};

use super::{CAPTURED_AT_MS, FXT_PID, Generation, Scenario};

/// 七端口假平台后端：单一结构按场景注入数据与失败。
#[derive(Debug)]
pub struct FakePlatform {
    pub(super) scenario: Scenario,
    /// 固定请求代数。
    generation: Generation,
    /// 快照时看到的基线身份（详情提供者据此判定 `NotFound`）。
    pub(super) baseline: ProcessIdentity,
    /// 进程控制在动作前重读得到的 current 身份（PID 复用注入点）。
    pub(super) current: ProcessIdentity,
    /// 实际执行过的动作（断言副作用计数）。
    pub(super) executed: RefCell<Vec<(Pid, ProcessAction)>>,
}

impl FakePlatform {
    /// 以场景构造；基线与 current 身份一致（同 PID、同合成 `start_time`）。
    ///
    /// # Errors
    /// 合成常量构造失败时返回错误（正常常量下不会发生）。
    pub fn new(scenario: Scenario) -> Result<Self, String> {
        let baseline = synthetic_identity(
            Pid::new(FXT_PID).map_err(|error| error.to_string())?,
            Some(captured_at()),
        );
        let current = baseline.clone();
        Ok(Self {
            scenario,
            generation: Generation::FIXTURE,
            baseline,
            current,
            executed: RefCell::new(Vec::new()),
        })
    }

    /// 注入 PID 复用：基线保持不变，把平台重读到的 current 改为给定身份。
    #[must_use]
    pub fn with_current(mut self, current: ProcessIdentity) -> Self {
        self.current = current;
        self
    }

    /// 固定 generation（fixture 同值约定）。
    pub const fn generation(&self) -> u64 {
        self.generation.get()
    }

    /// 实际执行的动作数量。
    pub fn executed_count(&self) -> usize {
        self.executed.borrow().len()
    }
}

pub(super) fn captured_at() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_millis(CAPTURED_AT_MS)
}

pub(super) fn issue(code: DiagnosticCode, message: &str) -> DiagnosticIssue {
    DiagnosticIssue::new(code, String::from(message))
}

pub(super) fn synthetic_identity(pid: Pid, start_time: Option<SystemTime>) -> ProcessIdentity {
    ProcessIdentity::new(
        pid,
        start_time,
        Some(std::path::PathBuf::from(
            "/opt/runquiry-fixtures/bin/fxt-daemon",
        )),
    )
}

impl FakePlatform {
    pub(super) fn inspect<T>(&self, data: Vec<T>) -> Inspection<Vec<T>> {
        let at = captured_at();
        match self.scenario {
            Scenario::Normal | Scenario::MalformedOutput => {
                Inspection::with_captured_at(Some(data), Vec::new(), at)
            }
            Scenario::Empty => Inspection::with_captured_at(Some(Vec::new()), Vec::new(), at),
            Scenario::Partial => Inspection::with_captured_at(
                Some(data),
                vec![issue(
                    DiagnosticCode::PermissionDenied,
                    "部分条目读取受限（合成场景）",
                )],
                at,
            ),
            Scenario::PermissionDenied => Inspection::with_captured_at(
                None,
                vec![issue(
                    DiagnosticCode::PermissionDenied,
                    "采集整体被拒绝（合成场景）",
                )],
                at,
            ),
            Scenario::ToolMissing => Inspection::with_captured_at(
                None,
                vec![issue(
                    DiagnosticCode::ExternalToolFailed,
                    "fxt CLI 缺失（合成场景）",
                )],
                at,
            ),
            Scenario::Timeout => Inspection::with_captured_at(
                None,
                vec![issue(DiagnosticCode::Timeout, "采集超时（合成场景）")],
                at,
            ),
        }
    }

    pub(super) fn capability_for(&self) -> CapabilityStatus {
        match self.scenario {
            Scenario::ToolMissing => {
                CapabilityStatus::Unavailable(String::from("容器运行时 CLI 未安装（合成场景）"))
            }
            Scenario::Normal
            | Scenario::Empty
            | Scenario::Partial
            | Scenario::PermissionDenied
            | Scenario::Timeout
            | Scenario::MalformedOutput => CapabilityStatus::Supported,
        }
    }
}
