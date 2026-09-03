//! 详情加载防抖（500ms）。
//!
//! 选择移动后延迟 [`DETAIL_DEBOUNCE_MS`] 再发起详情请求，避免按住方向键时
//! 每行都触发一次抓取（witr 的 `selectionDebounce` 语义）。时钟由调用方注入
//! （毫秒时间戳），本模块不读系统时间，可完全离线单测。
//!
//! 与代际的组合约定：`request` 记录发起时的代际；调用方在 [`DetailDebounce::poll`]
//! 拿到到期代际后，必须先用会话的 [`crate::session::WorkspaceSession::is_current`]
//! 检查——期间任何选择/筛选/工作区变化都会让旧代际失效，旧详情不得覆盖新选择。

/// 详情请求的防抖窗口。
pub const DETAIL_DEBOUNCE_MS: u64 = 500;

/// 一次待发起的详情请求。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pending {
    generation: runquiry_core::Generation,
    due_ms: u64,
}

/// 500ms 详情防抖状态机。
#[derive(Clone, Copy, Debug, Default)]
pub struct DetailDebounce {
    delay_ms: u64,
    pending: Option<Pending>,
}

impl DetailDebounce {
    /// 以标准 500ms 窗口创建防抖器。
    pub const fn new() -> Self {
        Self::with_delay(DETAIL_DEBOUNCE_MS)
    }

    /// 以自定义窗口创建防抖器（测试与将来按密度调整用）。
    pub const fn with_delay(delay_ms: u64) -> Self {
        Self {
            delay_ms,
            pending: None,
        }
    }

    /// 记录一次新选择：替换并使之前的待发起请求失效。
    pub const fn request(&mut self, generation: runquiry_core::Generation, now_ms: u64) {
        self.pending = Some(Pending {
            generation,
            due_ms: now_ms.saturating_add(self.delay_ms),
        });
    }

    /// 显式取消待发起的请求（如工作区已切换）。
    pub const fn cancel(&mut self) {
        self.pending = None;
    }

    /// 是否有待发起的请求。
    pub const fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// 取出到期请求的代际；每个请求只交付一次，未到期返回 `None`。
    pub fn poll(&mut self, now_ms: u64) -> Option<runquiry_core::Generation> {
        let pending = self.pending.as_ref().filter(|p| now_ms >= p.due_ms)?;
        let generation = pending.generation;
        self.pending = None;
        Some(generation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use runquiry_core::Generation;

    /// 到期前不交付，到期后恰好交付一次。
    #[test]
    fn fires_once_after_delay() {
        let mut debounce = DetailDebounce::new();
        let generation = Generation::first().next();
        debounce.request(generation, 1_000);

        assert!(debounce.is_pending());
        assert_eq!(debounce.poll(1_499), None, "500ms 窗口内不得发起");
        assert!(debounce.is_pending());
        assert_eq!(debounce.poll(1_500), Some(generation));
        assert!(!debounce.is_pending());
        assert_eq!(debounce.poll(2_000), None, "同一请求不得重复交付");
    }

    /// 新选择取消旧请求：只有最后一次选择的代际会被交付。
    #[test]
    fn new_request_supersedes_the_old_one() {
        let mut debounce = DetailDebounce::new();
        let mut first = Generation::first().next();
        let second = first.next();
        debounce.request(first, 0);
        debounce.request(second, 200);

        assert_eq!(debounce.poll(500), None, "旧请求应被新选择取消");
        assert_eq!(debounce.poll(700), Some(second));
    }

    /// cancel 立即作废待发起请求。
    #[test]
    fn cancel_discards_the_pending_request() {
        let mut debounce = DetailDebounce::new();
        debounce.request(Generation::first(), 0);
        debounce.cancel();
        assert!(!debounce.is_pending());
        assert_eq!(debounce.poll(9_999), None);
    }
}
