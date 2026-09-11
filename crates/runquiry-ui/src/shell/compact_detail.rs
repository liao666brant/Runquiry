//! 窄窗详情 Sheet：观察壳层实体，异步详情或快照变化后自动重绘。

use gpui_kit::{Context, Entity, Render, Subscription};

use super::AppShell;

pub(super) struct CompactDetail {
    shell: Entity<AppShell>,
    _subscription: Subscription,
}

impl CompactDetail {
    pub(super) fn new(shell: Entity<AppShell>, cx: &mut Context<'_, Self>) -> Self {
        let subscription = cx.observe(&shell, |_, _, cx| cx.notify());
        Self {
            shell,
            _subscription: subscription,
        }
    }
}

impl Render for CompactDetail {
    fn render(
        &mut self,
        _: &mut gpui_kit::Window,
        cx: &mut Context<'_, Self>,
    ) -> impl gpui_kit::IntoElement {
        self.shell.read(cx).detail_content(&self.shell, cx)
    }
}
