//! 契约级进程控制假实现：expected / current 身份比对语义。
//!
//! 语义对应 `runquiry_core::port::process::ProcessController` 契约修正
//! （A5 任务 5）：`execute` 收到的是确认流程持有的 expected 快照，假实现
//! 模拟平台在动作前按 PID 重读 current 身份并用 [`ProcessIdentity::same_process`]
//! 比较；不一致（含 current `start_time` 为 `None`）时返回
//! [`InspectError::ProcessChanged`] 且实际动作计数保持 0。

// 测试 crate 非 lib 目标，support 模块不对外导出：unreachable_pub 不适用。
#![allow(unreachable_pub)]
#![allow(dead_code)]

use std::cell::RefCell;

use runquiry_core::{
    CapabilityStatus, InspectError, Pid, ProcessAction, ProcessController, ProcessIdentity,
};

/// 进程控制假后端：`current` 模拟平台重读结果，`executed` 记录实际副作用。
#[derive(Debug)]
pub struct FakeController {
    capability: CapabilityStatus,
    /// 平台在动作前重读得到的 current identity。
    current: ProcessIdentity,
    /// 实际执行过的动作（用于断言副作用计数，而非仅返回值）。
    executed: RefCell<Vec<(Pid, ProcessAction)>>,
}

impl FakeController {
    /// 以平台重读到的 current 身份构造；能力状态默认 `Supported`。
    #[must_use]
    pub const fn new(current: ProcessIdentity) -> Self {
        Self {
            capability: CapabilityStatus::Supported,
            current,
            executed: RefCell::new(Vec::new()),
        }
    }

    /// 实际执行的动作记录（按执行顺序）。
    pub fn executed_actions(&self) -> std::cell::Ref<'_, Vec<(Pid, ProcessAction)>> {
        self.executed.borrow()
    }

    /// 实际执行的动作数量。
    pub fn executed_count(&self) -> usize {
        self.executed.borrow().len()
    }
}

impl ProcessController for FakeController {
    fn capability(&self) -> CapabilityStatus {
        self.capability.clone()
    }

    fn execute(
        &self,
        identity: &ProcessIdentity,
        action: ProcessAction,
    ) -> Result<(), InspectError> {
        // 契约：expected（调用方快照）与 current（平台重读）不一致即拒绝，
        // 不产生任何副作用；ProcessChanged 携带重读得到的 current 身份。
        if !identity.same_process(&self.current) {
            return Err(InspectError::ProcessChanged {
                identity: self.current.clone(),
            });
        }
        self.executed
            .borrow_mut()
            .push((self.current.pid(), action));
        Ok(())
    }
}
