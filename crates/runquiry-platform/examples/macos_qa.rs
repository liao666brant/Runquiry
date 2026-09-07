//! C1 macOS 真实采集与进程控制 QA 示例。
//!
//! 双 main 模式保证 Linux `cargo check --all-targets` 通过：非 macOS 平台
//! 打印说明即退出（cfg 代码在本机不参与编译）。macOS 侧只操作本程序自建
//! 并由 RAII 持有的 `sleep` 子进程；绝不打印环境变量值；不使用 sudo、不
//! 自动提权。
#![allow(clippy::print_stdout)] // QA 示例以控制台输出为交付物。
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("macos_qa 仅可在 macOS 上运行；Linux 侧验证见 tests/macos_*.rs。");
    println!(
        "Linux 侧可执行的验证：cargo test -p runquiry-platform --locked（macos_* 纯解析测试）。"
    );
}

#[cfg(target_os = "macos")]
mod macos {
    //! macOS 实机 QA：进程基线、端口、文件锁、来源证据与四类进程操作。
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    use runquiry_core::{
        FileInventory, NetworkInventory, Pid, ProcessAction, ProcessController,
        ProcessDetailsProvider, ProcessFileLocks, ProcessIdentity, ProcessInventory, Renice,
    };
    use runquiry_platform::macos::MacosPlatform;

    type QaResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    struct OwnedChild(Option<Child>);

    impl OwnedChild {
        fn spawn() -> Result<Self, std::io::Error> {
            Command::new("sleep")
                .arg("60")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map(|child| Self(Some(child)))
        }

        fn pid(&self) -> QaResult<u32> {
            self.0
                .as_ref()
                .map(Child::id)
                .ok_or_else(|| String::from("子进程已回收").into())
        }

        fn alive(&mut self) -> QaResult<bool> {
            Ok(self
                .0
                .as_mut()
                .ok_or_else(|| String::from("子进程已回收"))?
                .try_wait()?
                .is_none())
        }

        fn reap(&mut self) {
            if let Some(child) = self.0.as_mut() {
                let _ = child.kill();
                let _ = child.wait();
            }
            self.0 = None;
        }
    }

    impl Drop for OwnedChild {
        fn drop(&mut self) {
            self.reap();
        }
    }

    fn identity_of(platform: &MacosPlatform, pid: u32) -> QaResult<ProcessIdentity> {
        let listed = ProcessInventory::list(platform);
        let summaries = listed.data.ok_or("进程基线完全失败")?;
        summaries
            .into_iter()
            .find(|entry| entry.identity.pid() == Pid::new(pid)?)
            .map(|entry| entry.identity)
            .ok_or_else(|| format!("子进程 {pid} 不在基线（异常）").into())
    }

    pub(super) fn run() -> QaResult {
        let platform = MacosPlatform::new()?;
        println!("== Runquiry C1 macOS QA（真实采集） ==");

        // 1) 进程基线：自身排除生效。
        let listed = runquiry_core::ProcessInventory::list(&platform);
        let summaries = listed.data.ok_or("进程基线完全失败")?;
        println!(
            "进程基线: {} 条, 诊断 {} 条",
            summaries.len(),
            listed.issues.len()
        );
        let own = Pid::new(std::process::id())?;
        assert!(
            !summaries.iter().any(|entry| entry.identity.pid() == own),
            "自身 PID 不应出现在基线"
        );

        // 2) 端口与 Socket（lsof best-effort）。
        let ports = NetworkInventory::open_ports(&platform);
        let port_entries = ports.data.unwrap_or_default();
        println!(
            "开放端口: {} 条, 诊断 {} 条",
            port_entries.len(),
            ports.issues.len()
        );
        if let Some(first) = port_entries.iter().find(|entry| entry.pid.is_some()) {
            let pid = first.pid.ok_or("端口属主不可知（异常分支）")?;
            let sockets = NetworkInventory::sockets_of(&platform, pid);
            println!(
                "PID {pid} 的 Socket: {} 条, 诊断 {} 条",
                sockets.data.as_ref().map_or(0, |entries| entries.len()),
                sockets.issues.len()
            );
        }

        // 3) 进程详情（自身）。
        let own_details =
            ProcessDetailsProvider::details(&platform, &identity_of(&platform, own.get())?)?;
        let details = own_details.data.ok_or("自身详情完全失败")?;
        println!(
            "自身详情: cwd={:?}, fd_count={:?}, 环境变量条数={}",
            details.working_dir.as_ref().map_or_else(
                || String::from("(不可得)"),
                |path| path.display().to_string()
            ),
            details.fd_count,
            details.environment.len()
        );

        // 4) 自建子进程的四类操作 + renice。
        let mut child = OwnedChild::spawn()?;
        let pid = child.pid()?;
        println!("自建子进程 pid={pid}");
        let expected = identity_of(&platform, pid)?;

        for (action, label) in [
            (ProcessAction::Pause, "SIGSTOP"),
            (ProcessAction::Resume, "SIGCONT"),
            (ProcessAction::Renice(Renice::try_from(5)?), "renice 5"),
            (ProcessAction::Terminate, "SIGTERM"),
        ] {
            ProcessController::execute(&platform, &expected, action)?;
            println!("pid={pid} {label} ok");
            std::thread::sleep(Duration::from_millis(20));
        }
        // 等待子进程退出并回收。
        let deadline = Instant::now() + Duration::from_secs(2);
        while child.alive()? && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        child.reap();
        println!("pid={pid} cleanup=reaped");

        // 5) 身份拒绝：不可验证身份（start_time 不可得）必须零副作用拒绝。
        let unverifiable = ProcessIdentity::new(expected.pid(), None, None);
        match ProcessController::execute(&platform, &unverifiable, ProcessAction::Kill) {
            Err(error) => println!("身份拒绝回执: {error}"),
            Ok(()) => return Err("不可验证身份竟被 KILL 接受（异常）".into()),
        }

        // 6) 文件锁清单（lsof best-effort，无 root 也允许部分成功）。
        let locks = FileInventory::list(&platform);
        println!(
            "文件清单: {} 条, 诊断 {} 条",
            locks.data.as_ref().map_or(0, |entries| entries.len()),
            locks.issues.len()
        );
        if let Some(first) = locks.data.as_ref().and_then(|entries| entries.first()) {
            let own_locks = ProcessFileLocks::locks_of(&platform, first.pid)?;
            println!(
                "PID {} 的锁: {} 条",
                first.pid.get(),
                own_locks.data.as_ref().map_or(0, |entries| entries.len())
            );
        }
        println!("== C1 macOS QA 完成（进程与容器已清理） ==");
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    macos::run()
}
