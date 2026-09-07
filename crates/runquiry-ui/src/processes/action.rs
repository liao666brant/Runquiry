//! 进程动作确认、重复提交防护与异步结果门控。

use runquiry_core::{CapabilityStatus, Generation, InspectError, ProcessAction, ProcessIdentity};

/// 动作成功后详情区应采取的行为。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuccessDisposition {
    /// TERM/KILL 返回进程列表。
    ReturnToList,
    /// STOP/CONT/renice 保留并刷新同一详情。
    RefreshDetail,
}

/// 当前上下文仍接受的动作完成结果。
#[derive(Clone, Debug)]
pub enum ActionCompletion {
    /// 动作成功。
    Succeeded(SuccessDisposition),
    /// 动作失败，保留结构化错误供 UI 分类展示。
    Failed(InspectError),
}

/// 确认时冻结并交给后台的动作请求。
#[derive(Clone, Debug)]
pub struct ActionRequest {
    identity: ProcessIdentity,
    action: ProcessAction,
    generation: Generation,
}

impl ActionRequest {
    /// 用户确认时看到的进程身份。
    pub const fn identity(&self) -> &ProcessIdentity {
        &self.identity
    }

    /// 用户确认的具体动作。
    pub const fn action(&self) -> ProcessAction {
        self.action
    }

    /// 请求所属详情代际。
    pub const fn generation(&self) -> Generation {
        self.generation
    }

    fn matches(&self, other: &Self) -> bool {
        self.generation == other.generation
            && self.action == other.action
            && self.identity.same_process(&other.identity)
    }
}

/// 壳层持有的进程动作状态机。
#[derive(Debug)]
pub struct ProcessActionFlow {
    generation: Generation,
    pending: Option<ActionRequest>,
    in_flight: Option<ActionRequest>,
    last_error: Option<InspectError>,
}

impl Default for ProcessActionFlow {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessActionFlow {
    /// 创建空闲状态机。
    pub const fn new() -> Self {
        Self {
            generation: Generation::first(),
            pending: None,
            in_flight: None,
            last_error: None,
        }
    }

    /// 当前详情上下文代际。
    pub const fn generation(&self) -> Generation {
        self.generation
    }

    /// 是否已有确认或执行中的请求。
    pub const fn is_busy(&self) -> bool {
        self.pending.is_some() || self.in_flight.is_some()
    }

    /// 是否正在等待用户确认。
    pub const fn is_confirming(&self) -> bool {
        self.pending.is_some()
    }

    /// 是否已经确认并正在后台执行。
    pub const fn is_executing(&self) -> bool {
        self.in_flight.is_some()
    }

    /// 最近一次属于当前上下文的动作错误。
    pub const fn last_error(&self) -> Option<&InspectError> {
        self.last_error.as_ref()
    }

    /// 记录当前详情内的输入边界错误。
    pub fn report_error(&mut self, error: InspectError) {
        if self.in_flight.is_none() {
            self.pending = None;
            self.last_error = Some(error);
        }
    }

    /// 切换选择、查询或工作区时使旧完成结果失效。
    pub fn invalidate_context(&mut self) {
        let _ = self.generation.next();
        self.pending = None;
        self.last_error = None;
    }

    /// 从当前详情请求一次确认；能力不可用或已有请求时保持无副作用。
    pub fn request(
        &mut self,
        capability: &CapabilityStatus,
        identity: ProcessIdentity,
        action: ProcessAction,
    ) -> bool {
        if !capability.is_usable() || self.is_busy() {
            return false;
        }
        self.last_error = None;
        self.pending = Some(ActionRequest {
            identity,
            action,
            generation: self.generation,
        });
        true
    }

    /// 取消确认，不创建后台请求。
    pub fn cancel_confirmation(&mut self) -> bool {
        self.pending.take().is_some()
    }

    /// 确认并冻结请求；第二次确认不会产生重复执行。
    pub fn confirm(&mut self) -> Option<ActionRequest> {
        if self.in_flight.is_some() {
            return None;
        }
        let request = self.pending.take()?;
        self.in_flight = Some(request.clone());
        Some(request)
    }

    /// 应用后台结果；仅相同请求且详情代际仍有效时返回完成事件。
    pub fn complete(
        &mut self,
        request: &ActionRequest,
        result: Result<(), InspectError>,
    ) -> Option<ActionCompletion> {
        let matches_in_flight = self
            .in_flight
            .as_ref()
            .is_some_and(|active| active.matches(request));
        if !matches_in_flight {
            return None;
        }
        self.in_flight = None;
        if request.generation.is_stale(self.generation) {
            return None;
        }
        match result {
            Ok(()) => {
                self.last_error = None;
                Some(ActionCompletion::Succeeded(match request.action {
                    ProcessAction::Terminate | ProcessAction::Kill => {
                        SuccessDisposition::ReturnToList
                    }
                    ProcessAction::Pause | ProcessAction::Resume | ProcessAction::Renice(_) => {
                        SuccessDisposition::RefreshDetail
                    }
                }))
            }
            Err(error) => {
                self.last_error = Some(error.clone());
                Some(ActionCompletion::Failed(error))
            }
        }
    }
}
