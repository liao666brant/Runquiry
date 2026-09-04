//! QA 示例（B3 验收）：经生产 `ContainerInventory` 完成一次真实只读的
//! 容器列表 + 主机 PID + 富集读取，并打印摘要（无敏感值）。
//!
//! 用法：
//!
//! ```sh
//! cargo run -p runquiry-platform --locked --example container_qa
//! ```
//!
//! 环境限制如实呈现：未发现任何容器运行时 CLI 时输出能力说明，不伪造成功。

use std::io::Write as _;

use runquiry_core::{ContainerInventory, ContainerSummary, Pid};
use runquiry_platform::container::ContainerRuntimes;

fn main() {
    let mut out = std::io::stdout().lock();
    let inventory = ContainerRuntimes::new();
    let capability = inventory.capability();
    let _ = writeln!(out, "capability: {capability:?}");
    if !capability.is_usable() {
        // 如实输出环境限制（QA 主机未装容器 CLI 属预期结果）。
        let _ = writeln!(
            out,
            "环境限制：{}",
            capability.reason().unwrap_or("未知原因")
        );
        return;
    }

    let inspection = inventory.list();
    let _ = writeln!(out, "captured_at: {:?}", inspection.captured_at);
    for issue in &inspection.issues {
        let _ = writeln!(out, "issue [{}]: {}", issue.code().code(), issue.message());
    }
    let Some(items) = inspection.data else {
        let _ = writeln!(out, "无任何运行时返回数据（全部失败）");
        return;
    };
    if items.is_empty() {
        let _ = writeln!(out, "容器列表为空（各可用运行时均无容器）");
        return;
    }
    for item in &items {
        print_summary(&mut out, item);
    }
    // 对第一个容器走一次真实 host_pid + enrich（DETAIL_TIMEOUT）。
    let Some(first) = items.first() else {
        return;
    };
    let key = &first.key;
    match inventory.host_pid(key) {
        Ok(pid) => {
            let _ = writeln!(
                out,
                "host_pid({}) = {:?}",
                key.dedup_key(),
                pid.map(Pid::get)
            );
        }
        Err(err) => {
            let _ = writeln!(out, "host_pid({}) 失败: {err}", key.dedup_key());
        }
    }
    match inventory.enrich(key) {
        Ok(enrichment) => {
            let _ = writeln!(
                out,
                "enrich({}) started_at = {:?}",
                key.dedup_key(),
                enrichment.started_at
            );
        }
        Err(err) => {
            let _ = writeln!(out, "enrich({}) 失败: {err}", key.dedup_key());
        }
    }
}

fn print_summary(out: &mut std::io::StdoutLock<'_>, summary: &ContainerSummary) {
    let _ = writeln!(
        out,
        "container runtime={} id={} name={:?} image={:?} status={:?} health={:?} host_pid={:?}",
        summary.key.runtime,
        summary.key.id,
        summary.name,
        summary.image,
        summary.status,
        summary.health,
        summary.host_pid,
    );
}
