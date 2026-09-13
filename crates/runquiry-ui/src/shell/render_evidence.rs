//! 进程分析中可核对的来源、祖先、告警、Socket 与文件锁明细。

use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, IntoElement as _, ParentElement as _, SharedString, Styled as _, div,
};
use runquiry_core::{Analysis, ProcessSummary};
use rust_i18n::t;

use super::AppShell;

/// 子进程展示上限（超过折叠为「剩余 N 个」，witr PrintTree 同约定）。
const CHILD_LIMIT: usize = 10;

/// 非树形分组的可见条目上限。
const VISIBLE_ENTRIES: usize = 5;

impl AppShell {
    pub(super) fn render_evidence(analysis: &Analysis, cx: &App) -> AnyElement {
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
        let ancestry = ancestry_tree_lines(analysis);
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
            .child(ancestry_group(cx, t!("detail.ancestry"), &ancestry))
            .child(evidence_group(t!("detail.source"), &source))
            .child(evidence_group(t!("detail.warnings"), &warnings))
            .child(evidence_group(t!("detail.sockets"), &sockets))
            .child(evidence_group(t!("detail.files"), &files))
            .into_any_element()
    }
}

/// 祖先树文本行（witr `PrintTree` 同构）：祖先链自根向目标逐级 `└─` 缩进，
/// 目标的子进程以 `├─`/`└─` 挂在同一层级之下，超量折叠。
fn ancestry_tree_lines(analysis: &Analysis) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for (i, process) in analysis.ancestry.iter().enumerate() {
        let indent = "  ".repeat(i);
        let node = chain_name(process);
        if i == 0 {
            lines.push(format!("{indent}{node} (pid {})", process.identity.pid()));
        } else {
            lines.push(format!(
                "{indent}└─ {node} (pid {})",
                process.identity.pid()
            ));
        }
    }
    let base_indent = "  ".repeat(analysis.ancestry.len());
    let count = analysis.children.len();
    for (i, child) in analysis.children.iter().enumerate() {
        if i >= CHILD_LIMIT {
            lines.push(format!(
                "{base_indent}└─ {}",
                t!("detail.ancestry.more", count = count - CHILD_LIMIT)
            ));
            break;
        }
        let connector = if i == count - 1 { "└─" } else { "├─" };
        lines.push(format!(
            "{base_indent}{connector} {} (pid {})",
            chain_name(child),
            child.identity.pid()
        ));
    }
    lines
}

/// 节点显示名（witr `ChainName`：Command 优先，回退 `(unknown)`）。
fn chain_name(process: &ProcessSummary) -> &str {
    match process.command.is_empty() {
        false => &process.command,
        true => "(unknown)",
    }
}

fn evidence_group(label: impl Into<SharedString>, entries: &[String]) -> AnyElement {
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

/// 祖先树分组：等宽字体保证树形连接线对齐，且不截断行数（树是主信息）。
fn ancestry_group(cx: &App, label: impl Into<SharedString>, lines: &[String]) -> AnyElement {
    let mono = cx.theme().mono_font_family.clone();
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
        .children(lines.iter().map(|line| {
            div()
                .min_w_0()
                .text_sm()
                .font_family(mono.clone())
                .whitespace_normal()
                .child(line.clone())
        }))
        .when(lines.is_empty(), |group| {
            group.child(div().min_w_0().text_sm().whitespace_normal().child("—"))
        })
        .into_any_element()
}
