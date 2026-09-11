//! B3 测试共享助手：临时目录、假可执行脚本与子进程清理断言。
//!
//! 被各 `tests/command_*.rs` 与 `tests/container_*.rs` 经 `#[path]` 引入；
//! 独立编译为本文件自身的空测试 crate（无 `#[test]`）。脚本经 `/bin/sh`
//! 执行但**只**用于伪造被测 CLI 本身；Runquiry 代码路径不经 shell。
//! 全部路径在临时目录内，测试结束自清理。
//!
//! 测试 crate 非 lib 目标，support `模块不对外导出：unreachable_pub` 不适用；
//! 共享模块由各测试目标经 `#[path]` 取用，未用项不构成告警。
#![cfg(unix)]
#![allow(unreachable_pub)]
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// 测试统一错误类型（lints deny unwrap/expect，测试同样遵守）。
pub type TestResult = Result<(), Box<dyn std::error::Error>>;

/// 每个测试独享的临时目录；Drop 时整体删除。
#[derive(Debug)]
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// 在系统临时目录下创建唯一子目录。
    pub fn new(label: &str) -> Result<Self, std::io::Error> {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let name = format!(
            "runquiry-b3-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    /// 临时目录路径。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 写入一个可执行脚本（0o755），返回其绝对路径。
    pub fn write_executable(&self, name: &str, body: &str) -> Result<PathBuf, std::io::Error> {
        use std::os::unix::fs::PermissionsExt as _;
        let file = self.path.join(name);
        std::fs::write(&file, body)?;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755))?;
        // WSL2 内核在「写入后立即被子进程 execve」时会瞬态 ETXTBSY（实测
        // 并发下 plain ~6%，fsync 无效，空载愈合 ≤ ~0.9ms、8 线程高负载
        // ≤ ~5.7ms）；此处 2ms 等待吸收瞬时态，更长的残留由生产 spawn 的
        // ETXTBSY 有界重试（~75ms 窗口）兜底，两层合计覆盖实测分布。
        std::thread::sleep(std::time::Duration::from_millis(2));
        Ok(file)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// 生成假 CLI：`--version` 探测固定成功；其余子命令行为由 `body` 提供。
pub fn fake_cli(dir: &TempDir, name: &str, body: &str) -> Result<PathBuf, std::io::Error> {
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo 'fake {name} 1.0'\n  exit 0\nfi\n{body}\nexit 9\n"
    );
    dir.write_executable(name, &script)
}

/// 伪造某子命令输出固定文本（heredoc，避免引号转义）。
pub fn subcommand_response(subcommand: &str, output: &str) -> String {
    format!(
        "if [ \"$1\" = \"{subcommand}\" ]; then\n  cat <<'RUNQUIRY_EOF'\n{output}\nRUNQUIRY_EOF\n  exit 0\nfi\n"
    )
}

/// 伪造“程序挂起”：自报 PID 后 sleep，供超时/清理断言使用。
pub fn hang_script_body(pid_file: &Path, seconds: u32) -> String {
    format!(
        "#!/bin/sh\necho $$ > {}\nexec sleep {seconds}\n",
        pid_file.display()
    )
}

/// 读取脚本自报的 PID。
pub fn read_pid_file(path: &Path) -> Option<u32> {
    let text = std::fs::read_to_string(path).ok()?;
    text.trim().parse().ok()
}

/// 轮询等待 `/proc/<pid>` 消失（kill 后已被 wait 回收；僵尸进程仍存在）。
pub fn wait_until_gone(pid: u32, budget: std::time::Duration) -> bool {
    let entry = PathBuf::from(format!("/proc/{pid}"));
    let deadline = std::time::Instant::now() + budget;
    while std::time::Instant::now() < deadline {
        if !entry.exists() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    !entry.exists()
}

/// 全部 CLI 指向不存在路径的 [`runquiry_platform::container::RuntimeBinaries`]，
/// 供测试逐项覆盖被测运行时（避免真实宿主 CLI 干扰确定性）。
pub fn absent_binaries() -> runquiry_platform::container::RuntimeBinaries {
    let absent = |name: &str| {
        PathBuf::from(format!("/runquiry-b3-absent/{name}"))
            .display()
            .to_string()
    };
    runquiry_platform::container::RuntimeBinaries {
        docker: absent("docker"),
        podman: absent("podman"),
        nerdctl: absent("nerdctl"),
        crictl: absent("crictl"),
        incus: absent("incus"),
        lxd_client: absent("lxc"),
        lxd_daemon: absent("lxd"),
        lxc_ls: absent("lxc-ls"),
        lxc_info: absent("lxc-info"),
    }
}
