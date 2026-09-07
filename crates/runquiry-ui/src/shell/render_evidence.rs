//! 进程分析中可核对的来源、祖先、告警、Socket 与文件锁明细。

use gpui::prelude::FluentBuilder as _;
use gpui::{AnyElement, IntoElement as _, ParentElement as _, Styled as _, div};
use gpui_component::v_flex;
use runquiry_core::Analysis;
use rust_i18n::t;

use super::AppShell;

const VISIBLE_ENTRIES: usize = 5;

impl AppShell {
    pub(super) fn render_evidence(analysis: &Analysis) -> AnyElement {
        let source: Vec<String> = std::iter::once(analysis.source.source_type().code().to_owned())
            .chain(analysis.source.name().map(str::to_owned))
            .chain(analysis.source.description().map(str::to_owned))
            .chain(analysis.source.unit_file().map(str::to_owned))
            .chain(
                analysis
                    .source
                    .details()
                    .iter()
                    .map(|(key, value)| format!("{key}: {value}")),
            )
            .collect();
        let ancestry: Vec<String> = analysis
            .ancestry
            .iter()
            .map(|process| format!("{} · PID {}", process.command, process.identity.pid()))
            .collect();
        let warnings: Vec<String> = analysis
            .warnings
            .iter()
            .map(|warning| warning.message().to_owned())
            .collect();
        let sockets: Vec<String> = analysis
            .sockets
            .iter()
            .map(|socket| {
                format!(
                    "{:?} {}:{} · {}",
                    socket.protocol,
                    socket.address,
                    socket
                        .port
                        .map_or_else(|| "—".into(), |port| port.to_string()),
                    socket.state
                )
            })
            .collect();
        let files: Vec<String> = analysis
            .file_locks
            .iter()
            .map(|lock| {
                format!(
                    "{} · {:?}/{:?}",
                    lock.path.display(),
                    lock.lock_type,
                    lock.mode
                )
            })
            .collect();

        v_flex()
            .min_w_0()
            .gap_3()
            .child(evidence_group(t!("detail.ancestry"), &ancestry))
            .child(evidence_group(t!("detail.source"), &source))
            .child(evidence_group(t!("detail.warnings"), &warnings))
            .child(evidence_group(t!("detail.sockets"), &sockets))
            .child(evidence_group(t!("detail.files"), &files))
            .into_any_element()
    }
}

fn evidence_group(label: impl Into<gpui::SharedString>, entries: &[String]) -> AnyElement {
    v_flex()
        .min_w_0()
        .gap_1()
        .child(
            div()
                .min_w_0()
                .text_xs()
                .whitespace_normal()
                .child(label.into()),
        )
        .children(entries.iter().take(VISIBLE_ENTRIES).map(|entry| {
            div()
                .min_w_0()
                .text_sm()
                .whitespace_normal()
                .child(entry.clone())
        }))
        .when(entries.is_empty(), |group| {
            group.child(div().min_w_0().text_sm().whitespace_normal().child("—"))
        })
        .into_any_element()
}
