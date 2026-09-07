//! 后台采集与 generation 门控。

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{AppContext as _, Context, Task};

use super::AppShell;
use crate::backend::{WorkspaceResultGate, WorkspaceSnapshot};

impl AppShell {
    /// 发起当前工作区刷新；所有同步采集都在后台执行器运行。
    pub fn refresh_active(&mut self, cx: &mut Context<'_, Self>) {
        let workspace = self.active_workspace();
        let Some(generation) = self.session.active_session_mut().try_refresh() else {
            return;
        };
        let gate = WorkspaceResultGate::new(workspace, generation);
        self.refresh_started = Some((gate, Instant::now()));
        let backend = Arc::clone(&self.backend);
        let work = cx.background_spawn(async move { backend.load(workspace) });
        self.refresh_task = Some(cx.spawn(async move |shell, cx| {
            let snapshot = work.await;
            let _ = shell.update(cx, |shell, cx| shell.apply_refresh(gate, snapshot, cx));
        }));
    }

    /// 动作成功后的强制刷新：先使旧采集失效，再立即开始新代际。
    pub(super) fn force_refresh_active(&mut self, cx: &mut Context<'_, Self>) {
        if let Some((gate, _)) = self.refresh_started.take() {
            self.session
                .session_mut(gate.workspace())
                .abort_refresh(gate.generation());
        }
        self.session.active_session_mut().invalidate_context();
        self.refresh_active(cx);
    }

    fn apply_refresh(
        &mut self,
        gate: WorkspaceResultGate,
        snapshot: WorkspaceSnapshot,
        cx: &mut Context<'_, Self>,
    ) {
        let workspace = snapshot.workspace();
        if !gate.accepts_session(&self.session, &snapshot) {
            return;
        }
        let Some((active_gate, started)) = self.refresh_started else {
            return;
        };
        if active_gate != gate {
            return;
        }
        self.data.apply(snapshot, cx);
        if self
            .session
            .session_mut(workspace)
            .finish_refresh(gate.generation(), started.elapsed())
        {
            self.refresh_started = None;
            cx.notify();
        }
    }

    pub(crate) const fn active_interval(&self) -> Duration {
        self.session.active_session().refresh_interval()
    }

    fn auto_refresh(&mut self, cx: &mut Context<'_, Self>) {
        if !self.detail_loading {
            self.refresh_active(cx);
        }
    }
}

pub(super) fn spawn_auto_refresh(cx: &Context<'_, AppShell>) -> Task<()> {
    cx.spawn(async move |shell, cx| {
        loop {
            let interval = shell
                .update(cx, |shell, _| shell.active_interval())
                .unwrap_or(Duration::from_secs(3));
            cx.background_executor().timer(interval).await;
            if shell.update(cx, AppShell::auto_refresh).is_err() {
                break;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use crate::{AppSession, WorkspaceId};

    #[test]
    fn action_refresh_supersedes_an_inflight_load_instead_of_being_dropped() {
        let mut session = AppSession::new();
        let first = session.active_session_mut().try_refresh();
        assert!(first.is_some());
        let Some(first) = first else { return };
        assert!(session.active_session().is_refreshing());

        assert!(
            session
                .session_mut(WorkspaceId::Processes)
                .abort_refresh(first)
        );
        session.active_session_mut().invalidate_context();
        let replacement = session.active_session_mut().try_refresh();

        assert!(replacement.is_some(), "动作后的刷新不得被重入门吞掉");
        assert!(replacement.is_some_and(|generation| generation != first));
        assert!(session.active_session().is_refreshing());
    }
}
