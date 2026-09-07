//! 表格、筛选与调查入口的事件接线。

use std::sync::Arc;
use std::time::Duration;

use gpui::{AppContext as _, Context, ParentElement as _, Window};
use gpui_component::{WindowExt as _, table::TableEvent};
use runquiry_core::{Generation, InspectError, ProcessIdentity, Resolution};

use super::{AppShell, compact_detail::CompactDetail};
use crate::backend::InvestigationTarget;
use crate::processes::QueryOutcome;
use crate::processes::TargetKind;

const DETAIL_DELAY: Duration = Duration::from_millis(500);

impl AppShell {
    pub(crate) fn set_target_kind(&mut self, kind: TargetKind, cx: &mut Context<'_, Self>) {
        if self.target_kind == kind {
            return;
        }
        self.target_kind = kind;
        let _ = self.query_generation.next();
        self.query_outcome = None;
        self.query_error = None;
        self.analysis = None;
        self.detail_loading = false;
        cx.notify();
    }

    pub(crate) fn on_process_table(
        &mut self,
        event: &TableEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let (TableEvent::SelectRow(row) | TableEvent::DoubleClickedRow(row)) = event else {
            return;
        };
        self.invalidate_active_refresh();
        self.data.processes.select_row(*row);
        self.analysis = None;
        self.detail_loading = false;
        self.query_error = None;
        self.query_outcome = None;
        let selection = self
            .data
            .processes
            .selected()
            .map(|identity| format!("{}:{:?}", identity.pid(), identity.start_time()));
        self.session.active_session_mut().select(selection);
        Self::open_compact_detail(window, cx);
        let Some(request) = self.data.processes.begin_detail() else {
            return;
        };
        self.detail_request = Some(request.clone());
        self.detail_loading = true;
        let backend = Arc::clone(&self.backend);
        let work = cx.background_spawn({
            let identity = request.identity().clone();
            async move {
                std::thread::sleep(DETAIL_DELAY);
                backend.analyze(&identity)
            }
        });
        self.detail_task = Some(cx.spawn(async move |shell, cx| {
            let result = work.await;
            let _ = shell.update(cx, |shell, cx| {
                if shell.data.processes.accepts(&request) {
                    shell.detail_loading = false;
                    match result {
                        Ok(analysis) => {
                            shell.analysis = Some(analysis);
                            shell.query_error = None;
                        }
                        Err(error) => shell.query_error = Some(error),
                    }
                    cx.notify();
                }
            });
        }));
    }

    pub(crate) fn submit_query(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let raw = self.query_input.read(cx).value().to_string();
        let target = match self.target_kind.parse(&raw, false) {
            Ok(target) => target,
            Err(error) => {
                let _ = self.query_generation.next();
                self.detail_loading = false;
                self.query_error = Some(error);
                self.query_outcome = None;
                Self::open_compact_detail(window, cx);
                cx.notify();
                return;
            }
        };
        let generation = self.query_generation.next();
        self.query_error = None;
        self.query_outcome = None;
        self.analysis = None;
        self.detail_loading = false;
        self.data.processes.privacy_mut().reset();
        Self::open_compact_detail(window, cx);
        let backend = Arc::clone(&self.backend);
        let work = cx.background_spawn(async move { backend.resolve(&target) });
        self.detail_task = Some(cx.spawn(async move |shell, cx| {
            let result = work.await;
            let _ = shell.update(cx, |shell, cx| {
                shell.apply_resolution(generation, result, cx);
            });
        }));
    }

    fn apply_resolution(
        &mut self,
        generation: Generation,
        result: Result<Resolution<InvestigationTarget>, runquiry_core::InspectError>,
        cx: &mut Context<'_, Self>,
    ) {
        if generation.is_stale(self.query_generation) {
            return;
        }
        match result {
            Ok(resolution) => {
                let outcome = QueryOutcome::from_resolution(resolution);
                let selected = outcome.selected().cloned();
                self.query_outcome = Some(outcome);
                if let Some(InvestigationTarget::Process(identity)) = selected {
                    self.schedule_query_analysis(identity, generation, cx);
                }
            }
            Err(InspectError::NotFound { .. }) => {
                self.detail_loading = false;
                self.query_outcome = Some(QueryOutcome::empty());
            }
            Err(error) => {
                self.detail_loading = false;
                self.query_error = Some(error);
            }
        }
        cx.notify();
    }

    pub(crate) fn choose_candidate(
        &mut self,
        target: InvestigationTarget,
        cx: &mut Context<'_, Self>,
    ) {
        let generation = self.query_generation.next();
        self.query_outcome = Some(QueryOutcome::Unique(target.clone()));
        self.analysis = None;
        self.data.processes.privacy_mut().reset();
        if let InvestigationTarget::Process(identity) = target {
            self.schedule_query_analysis(identity, generation, cx);
        } else {
            self.detail_loading = false;
            cx.notify();
        }
    }

    fn schedule_query_analysis(
        &mut self,
        identity: ProcessIdentity,
        generation: Generation,
        cx: &Context<'_, Self>,
    ) {
        let backend = Arc::clone(&self.backend);
        let expected = identity.clone();
        self.detail_loading = true;
        let work = cx.background_spawn(async move {
            std::thread::sleep(DETAIL_DELAY);
            backend.analyze(&identity)
        });
        self.detail_task = Some(cx.spawn(async move |shell, cx| {
            let result = work.await;
            let _ = shell.update(cx, |shell, cx| {
                let selected_matches = shell
                    .query_outcome
                    .as_ref()
                    .and_then(QueryOutcome::selected)
                    .is_some_and(|current| {
                        matches!(current, InvestigationTarget::Process(identity) if identity.same_process(&expected))
                    });
                if !generation.is_stale(shell.query_generation) && selected_matches {
                    shell.detail_loading = false;
                    match result {
                        Ok(analysis) => {
                            shell.analysis = Some(analysis);
                            shell.query_error = None;
                        }
                        Err(error) => shell.query_error = Some(error),
                    }
                    cx.notify();
                }
            });
        }));
    }

    pub(crate) fn focus_query(&self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.query_input
            .update(cx, |input, cx| input.focus(window, cx));
    }

    pub(crate) fn open_compact_detail(window: &mut Window, cx: &mut Context<'_, Self>) {
        if u32::from(window.bounds().size.width) >= 1_100 {
            return;
        }
        let shell = cx.entity();
        let detail = cx.new(|cx| CompactDetail::new(shell, cx));
        window.open_sheet(cx, move |sheet, _, _| {
            sheet
                .title(rust_i18n::t!("detail.title").to_string())
                .child(detail.clone())
        });
    }
}
