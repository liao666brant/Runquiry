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
        CapabilityStatus, FileInventory, NetworkInventory, ProcessController,
        ProcessDetailsProvider, ProcessInventory, SourceEvidenceProvider,
    };
    use runquiry_platform::windows::WindowsPlatform;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        IsWow64Process, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    /// 能力状态的人读摘要（reason 原样打印；均为稳定原因键，非敏感值）。
    fn capability_line(status: &CapabilityStatus) -> String {
        match status {
            CapabilityStatus::Supported => String::from("Supported"),
            CapabilityStatus::Partial(reason) => format!("Partial: {reason}"),
            CapabilityStatus::Unsupported(reason) => format!("Unsupported: {reason}"),
            CapabilityStatus::Unavailable(reason) => format!("Unavailable: {reason}"),
        }
    }

    /// 探测进程是否为 32 位（WOW64）；句柄不可得或查询失败为 `None`。
    fn is_wow64_process(pid: u32) -> Option<bool> {
        // SAFETY:OpenProcess 仅读取标量参数；返回 NULL 表示失败（部分系统
        // 进程连有限查询权限也不可得，调用方跳过该 PID）。
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return None;
        }
        let mut wow64: windows_sys::core::BOOL = 0;
        // SAFETY:handle 为上一步成功返回的有效句柄；wow64 为栈上 i32 出参。
        let ok = unsafe { IsWow64Process(handle, &mut wow64) };
        // SAFETY:handle 为本函数打开的查询句柄，此处为唯一关闭点。
        unsafe {
            CloseHandle(handle);
        }
        (ok != 0).then_some(wow64 != 0)
    }

    /// 只读探测容器 CLI 是否可用（`--version`，无副作用）。
    fn cli_present(cli: &str) -> bool {
        std::process::Command::new(cli)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
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

        // 5) 受保护进程部分结果：命中已知受保护系统进程时读取详情，验证
        //    「Ok + 部分字段缺失 + 结构化诊断」契约；只打印计数与诊断码，
        //    不打印任何值（witr：部分结果不伪装为失败或完整数据）。
        let protected = ["lsass", "services", "winlogon", "csrss", "smss", "wininit"];
        let hits: Vec<_> = summaries
            .iter()
            .filter(|summary| {
                let name = summary.command.to_ascii_lowercase();
                protected.iter().any(|prefix| name.starts_with(prefix))
            })
            .take(2)
            .collect();
        if hits.is_empty() {
            println!("受保护进程: 基线未命中（本机场景不适用）");
        }
        for summary in hits {
            let pid = summary.identity.pid().get();
            match ProcessDetailsProvider::details(&platform, &summary.identity) {
                Ok(inspection) => match inspection.data {
                    Some(details) => {
                        let codes: Vec<String> = inspection
                            .issues
                            .iter()
                            .map(|issue| format!("{:?}", issue.code()))
                            .collect();
                        println!(
                            "受保护进程 {}（PID {pid}）: 诊断 {} 条, 环境块 {} 项, 工作目录 {}, 内存计数 {}",
                            summary.command,
                            codes.len(),
                            details.environment.len(),
                            details.working_dir.is_some(),
                            details.memory.is_some(),
                        );
                        println!("  诊断码: {}", codes.join(", "));
                    }
                    None => println!("受保护进程（PID {pid}）: 详情数据缺失"),
                },
                Err(_) => println!("受保护进程（PID {pid}）: 详情不可得（身份变化或已退出）"),
            }
        }

        // 6) 32 位进程 PEB32 读取：基线内 IsWow64Process 探测首个 32 位
        //    进程并读取详情（环境块 / 工作目录为 PEB 派生字段）；无 32 位
        //    进程时明确报告「未验证」，不静默跳过。
        let wow64_hit = summaries
            .iter()
            .find(|summary| is_wow64_process(summary.identity.pid().get()) == Some(true));
        match wow64_hit {
            Some(summary) => {
                let pid = summary.identity.pid().get();
                match ProcessDetailsProvider::details(&platform, &summary.identity) {
                    Ok(inspection) => match inspection.data {
                        Some(details) => println!(
                            "32 位进程（PID {pid}）PEB32 详情: 环境块 {} 项, 工作目录 {}, 诊断 {} 条",
                            details.environment.len(),
                            details.working_dir.is_some(),
                            inspection.issues.len(),
                        ),
                        None => println!("32 位进程（PID {pid}）: 详情数据缺失"),
                    },
                    Err(_) => println!("32 位进程（PID {pid}）: 详情不可得"),
                }
            }
            None => println!("32 位进程: 本机基线未发现（PEB32 场景未验证）"),
        }

        // 7) 容器运行时 CLI 存在性（只读探测）：覆盖「未安装」场景的实机
        //    证据；CLI 存在时「未启动 / 已启动」场景另行验证。
        for cli in ["docker", "podman", "nerdctl"] {
            println!(
                "容器 CLI {cli}: {}",
                if cli_present(cli) {
                    "存在"
                } else {
                    "未安装"
                }
            );
        }

        println!("== C2 Windows QA 完成（全程只读） ==");
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    windows::run()
}
