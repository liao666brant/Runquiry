//! C2 Windows 真实采集 QA 示例（只读）。
//!
//! 双 main 模式保证 Linux `cargo check --all-targets` 通过：非 Windows
//! 平台打印说明即退出（cfg 代码在本机不参与编译）。全部动作为只读采集：
//! 进程基线、端口、SCM 服务证据与 Unsupported 能力状态；敏感值（环境变量
//! 值、服务描述、二进制路径）只报计数，绝不打印。不使用管理员权限、不
//! 自动提权、不触发任何写入或控制动作。
#![allow(clippy::print_stdout)] // QA 示例以控制台输出为交付物。

#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

#[cfg(not(target_os = "windows"))]
fn main() {
    println!("windows_qa 仅可在 Windows 上运行；Linux 侧验证见 tests/windows_*.rs。");
    println!(
        "Linux 侧可执行的验证：cargo test -p runquiry-platform --locked（windows_* 纯解析测试）。"
    );
    std::process::exit(0);
}

#[cfg(target_os = "windows")]
mod windows {
    //! Windows 实机 QA：只读采集与 Unsupported 能力状态核对。
    use runquiry_core::{
        CapabilityStatus, FileInventory, NetworkInventory, ProcessController, ProcessInventory,
        SourceEvidenceProvider,
    };
    use runquiry_platform::windows::WindowsPlatform;

    /// 能力状态的人读摘要（reason 原样打印；均为稳定原因键，非敏感值）。
    fn capability_line(status: &CapabilityStatus) -> String {
        match status {
            CapabilityStatus::Supported => String::from("Supported"),
            CapabilityStatus::Partial(reason) => format!("Partial: {reason}"),
            CapabilityStatus::Unsupported(reason) => format!("Unsupported: {reason}"),
            CapabilityStatus::Unavailable(reason) => format!("Unavailable: {reason}"),
        }
    }

    pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
        let platform = WindowsPlatform::new()?;
        println!("== Runquiry C2 Windows QA（只读采集） ==");

        // 1) 进程基线计数（自身排除生效与否由基线不含自身保证）。
        let listed = ProcessInventory::list(&platform);
        let summaries = listed.data.unwrap_or_default();
        println!(
            "进程基线: {} 条, 诊断 {} 条",
            summaries.len(),
            listed.issues.len()
        );

        // 2) 端口清单计数（属主不可知的条目计入诊断，不打印地址）。
        let ports = NetworkInventory::open_ports(&platform);
        println!(
            "开放端口: {} 条, 诊断 {} 条",
            ports.data.as_ref().map_or(0, |entries| entries.len()),
            ports.issues.len()
        );

        // 3) SCM 服务证据计数：以前若干基线条目为祖先链（只读采集）；
        //    键值内容（服务描述 / 二进制路径等）只计总数，不打印。
        let ancestry: Vec<_> = summaries.iter().take(8).cloned().collect();
        let evidence = SourceEvidenceProvider::evidence(&platform, &ancestry);
        let key_total: usize = evidence
            .windows_service_by_pid
            .iter()
            .map(|(_, kv)| kv.len())
            .sum();
        println!(
            "SCM 服务证据: 覆盖 {} 个 PID, 键值 {} 条（值不打印）",
            evidence.windows_service_by_pid.len(),
            key_total
        );

        // 4) Unsupported 能力状态（parity §10：文件锁与进程操作整类不可用）。
        println!(
            "FileInventory 能力: {}",
            capability_line(&FileInventory::capability(&platform))
        );
        println!(
            "ProcessController 能力: {}",
            capability_line(&ProcessController::capability(&platform))
        );
        println!("== C2 Windows QA 完成（全程只读） ==");
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    windows::run()
}