//! B2 Linux 真实采集 QA 示例（B2 完成证据：普通用户运行）。
//!
//! 用生产适配器读取当前进程（自身）与一个自建短生命周期子进程，输出进程
//! 基线、FD、端口与部分权限结果的可观察摘要。
//!
//! 安全红线：**绝不打印环境变量值与敏感参数**——环境只输出计数；
//! 不读其他进程的环境值；不使用 sudo，不自动提权。
#![allow(clippy::print_stdout, clippy::print_stderr)] // QA 示例以控制台输出为交付物

// 双 main 模式保证非 Linux 平台 `cargo test --locked` 可编译（与 windows_qa
// 的门控模式一致）。
#[cfg(not(target_os = "linux"))]
fn main() {
    println!("linux_qa 仅可在 Linux 上运行；Windows 侧验证见 tests/windows_*.rs。");
    println!(
        "本机可执行的验证：cargo test -p runquiry-platform --locked（windows_* 纯解析测试）。"
    );
}

#[cfg(target_os = "linux")]
use std::process::Command;

#[cfg(target_os = "linux")]
use runquiry_core::{
    FileInventory, NetworkInventory, Pid, ProcessDetailsProvider, ProcessFileLocks,
    ProcessIdentity, ProcessInventory,
};
#[cfg(target_os = "linux")]
use runquiry_platform::linux::LinuxPlatform;

#[cfg(target_os = "linux")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let platform = LinuxPlatform::new()?;
    println!("== Runquiry B2 Linux QA（真实采集） ==");
    println!("proc 根目录: {}", platform.proc_root().display());

    // 1) 进程基线：自身与采集期辅助进程已被排除策略过滤。
    let listed = ProcessInventory::list(&platform);
    let summaries = listed.data.ok_or("进程基线完全失败（无数据）")?;
    println!(
        "进程基线: {} 条, 诊断 {} 条",
        summaries.len(),
        listed.issues.len()
    );
    for issue in listed.issues.iter().take(5) {
        println!("  诊断 [{}] {}", issue.code().code(), issue.message());
    }
    let own = Pid::new(std::process::id())?;
    println!(
        "自身排除: PID {own} {}",
        if summaries.iter().any(|entry| entry.identity.pid() == own) {
            "仍在基线（异常）"
        } else {
            "已被排除（符合预期）"
        }
    );
    // 健康标签分布（真实数据下的 parity 健康计算观察；不含用户数据）。
    let mut high_cpu = 0usize;
    let mut high_mem = 0usize;
    let mut zombie = 0usize;
    for entry in &summaries {
        match entry.health {
            runquiry_core::HealthStatus::HighCpu => high_cpu += 1,
            runquiry_core::HealthStatus::HighMem => high_mem += 1,
            runquiry_core::HealthStatus::Zombie => zombie += 1,
            _ => {}
        }
    }
    println!("健康标签: high-cpu {high_cpu} / high-mem {high_mem} / zombie {zombie}");

    // 2) 自身详情（身份 start_time 未知时按 QA 约定直读当前数据）。
    let own_identity = ProcessIdentity::new(own, None, None);
    let details_inspection = ProcessDetailsProvider::details(&platform, &own_identity)?;
    let details = details_inspection
        .data
        .ok_or("自身详情完全失败（无数据）")?;
    println!(
        "自身详情: comm={:?}, fd={:?} / limit={:?}, cwd={:?}, 环境变量 {} 个，诊断 {} 条（不报环境值）",
        summaries
            .iter()
            .find(|entry| entry.identity.pid() == own)
            .map_or("<已排除>", |entry| entry.command.as_str()),
        details.fd_count,
        details.fd_limit,
        details.working_dir,
        details.environment.len(),
        details_inspection.issues.len()
    );

    // 3) 自建短生命周期子进程：spawn sleep 3 秒后采集，结束后回收。
    let mut child = Command::new("sleep").arg("3").spawn()?;
    let child_pid = Pid::new(child.id())?;
    print_child_details(&platform, child_pid)?;
    let _ = child.wait()?;

    // 4) 端口清单：部分权限结果以诊断呈现，无主端口保留为 pid None。
    let ports = NetworkInventory::open_ports(&platform);
    if let Some(entries) = ports.data.as_ref() {
        let attributed = entries.iter().filter(|entry| entry.pid.is_some()).count();
        println!(
            "开放端口: {} 条 (归因 {} 条, 无主 {} 条), 诊断 {} 条",
            entries.len(),
            attributed,
            entries.len() - attributed,
            ports.issues.len()
        );
        for entry in entries.iter().take(5) {
            println!(
                "  {:?} {}:{} state={} pid={:?}",
                entry.protocol, entry.address, entry.port, entry.state, entry.pid
            );
        }
    }
    for issue in ports.issues.iter().take(5) {
        println!("  诊断 [{}] {}", issue.code().code(), issue.message());
    }

    print_file_inventory_qa(&platform)?;
    println!("== QA 结束（普通用户权限；未打印任何环境变量值） ==");
    Ok(())
}

/// 文件清单只输出分类计数，不暴露全量路径或进程信息。可选首个参数指向
/// 调用方自行创建的受控目标；默认查询自身可执行文件。
#[cfg(target_os = "linux")]
fn print_file_inventory_qa(platform: &LinuxPlatform) -> Result<(), Box<dyn std::error::Error>> {
    let inventory = FileInventory::list(platform);
    let inventory_entries = inventory.data.as_deref().unwrap_or_default();
    let plain_count = inventory_entries
        .iter()
        .filter(|entry| entry.lock.is_none())
        .count();
    let lock_count = inventory_entries
        .iter()
        .filter(|entry| entry.lock.is_some())
        .count();
    println!(
        "文件清单: 普通 FD {plain_count} 条, 真实锁 {lock_count} 条, 诊断 {} 条",
        inventory.issues.len()
    );

    let target = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .map_or_else(std::env::current_exe, Ok)?;
    let holders = FileInventory::holders(platform, &target);
    let target_label = target.file_name().map_or_else(
        || String::from("<受控目标>"),
        |name| name.to_string_lossy().into_owned(),
    );
    let target_entries = holders.data.as_deref().unwrap_or_default();
    let target_plain = target_entries
        .iter()
        .filter(|entry| entry.lock.is_none())
        .count();
    let target_locks = target_entries
        .iter()
        .filter(|entry| entry.lock.is_some())
        .count();
    println!(
        "受控目标 {target_label}: 普通 FD {target_plain} 条, 真实锁 {target_locks} 条, 诊断 {} 条",
        holders.issues.len()
    );
    if let Some(lock_pid) = target_entries
        .iter()
        .find(|entry| entry.lock.is_some())
        .map(|entry| entry.pid)
    {
        let owned = ProcessFileLocks::locks_of(platform, lock_pid);
        println!(
            "受控锁进程: locks_of {} 条, 诊断 {} 条",
            owned.data.as_ref().map_or(0, Vec::len),
            owned.issues.len()
        );
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn print_child_details(
    platform: &LinuxPlatform,
    child_pid: Pid,
) -> Result<(), Box<dyn std::error::Error>> {
    let inspected =
        ProcessDetailsProvider::details(platform, &ProcessIdentity::new(child_pid, None, None))?;
    let details = inspected.data.ok_or("子进程详情完全失败（无数据）")?;
    println!(
        "子进程 {child_pid}: fd={:?} / limit={:?}, 打开文件 {:?}, 诊断 {} 条",
        details.fd_count,
        details.fd_limit,
        details.open_files,
        inspected.issues.len()
    );
    let sockets = NetworkInventory::sockets_of(platform, child_pid);
    println!(
        "子进程 socket: {} 条, 诊断 {} 条",
        sockets.data.as_ref().map_or(0, Vec::len),
        sockets.issues.len()
    );
    Ok(())
}
