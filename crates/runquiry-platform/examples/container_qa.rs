//! QA 示例（B3 验收）：经生产 `ContainerInventory` 完成一次真实只读的
//! 容器列表 + 经归属验证的主机 PID + 富集读取，并打印摘要。
//!
//! 用法：
//!
//! ```sh
//! cargo run -p runquiry-platform --locked --example container_qa
//! ```
//!
//! 环境限制如实呈现：未发现任何容器运行时 CLI 时输出能力说明，不伪造成功。

use std::io::Write as _;

use runquiry_core::{ContainerInventory, ContainerKey, ContainerSummary, InspectError, Pid};
use runquiry_platform::container::ContainerRuntimes;
#[cfg(target_os = "linux")]
use runquiry_platform::linux::LinuxPlatform;

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
    // 对第一个容器走一次真实 verified_host_pid + enrich（DETAIL_TIMEOUT）。
    let Some(first) = items.first() else {
        return;
    };
    let key = &first.key;
    match verified_host_pid(&inventory, key) {
        Ok(pid) => {
            let _ = writeln!(
                out,
                "verified_host_pid({}) = {:?}",
                key.dedup_key(),
                pid.map(Pid::get)
            );
        }
        Err(err) => {
            let _ = writeln!(out, "verified_host_pid({}) 失败: {err}", key.dedup_key());
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

#[cfg(target_os = "linux")]
fn verified_host_pid(
    inventory: &ContainerRuntimes,
    key: &ContainerKey,
) -> Result<Option<Pid>, InspectError> {
    let verifier = LinuxPlatform::new().map_err(|error| InspectError::Unsupported {
        reason: format!("无法初始化 Linux 容器归属验证器：{error}"),
    })?;
    inventory.verified_host_pid(key, &verifier)
}

#[cfg(not(target_os = "linux"))]
fn verified_host_pid(
    _inventory: &ContainerRuntimes,
    _key: &ContainerKey,
) -> Result<Option<Pid>, InspectError> {
    Ok(None)
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
