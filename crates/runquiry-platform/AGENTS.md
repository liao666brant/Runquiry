# Runquiry Platform 平台层

[crates](../) / runquiry-platform

## 模块职责

平台采集与控制层：Linux/Windows 采集器、容器运行时集成、进程控制（关闭类与暂停/恢复/renice）与文件定位、外部命令执行。`src/command/` 是受限命令执行器，`src/container/` 集成七种容器运行时，`src/linux/` 与 `src/windows/` 分别提供两个平台的全部端口实现。

约束：只依赖 runquiry-core，为其端口（trait）提供实现；UI 不得绕过本层直接读取 /proc、调用 Win32 API 或运行 lsof/容器 CLI。全部外部命令（容器 CLI、QA 假 CLI）只能经 `StdCommandRunner`，绝不经过 shell。macOS 与其它目标已移出 v1 范围。

## 入口与启动

无独立入口。库 crate，`src/lib.rs` 声明跨平台的 `pub mod command`、`pub mod container` 与 cfg 门控的 `pub mod linux`（`target_os = "linux"`）/ `pub mod windows`（`target_os = "windows"`）；非这两个目标由 `compile_error!("Runquiry 仅支持 Linux 与 Windows 目标平台")` 显式拒绝。平台结构体对外的公开路径为 `runquiry_platform::linux::LinuxPlatform` 与 `runquiry_platform::windows::WindowsPlatform`。

## 对外接口

- `command::StdCommandRunner`：`CommandRunner` 生产实现，`CommandFailure` 区分 Spawn/Timeout/OutputLimit/Cancelled 并统一映射为 `InspectError::ExternalTool`。程序名 + 独立 argv，不经 shell，stdin 为 null；stdout/stderr 各限 8 MiB（上限与超时来自 core `port/command.rs`），超限、取消与超时均终止进程组并 wait，读取错误不伪装成功；Unix 下以进程组启动（`process_group(0)`）并按 `-pgid` 回收；脚本启动对 ETXTBSY 有小次数有界重试；`CancellationToken` 支持运行中取消。sudo 下 Podman/nerdctl 经校验后恢复原用户 UID/GID 与 rootless 环境（`original_user`，`cfg(unix)`），Docker 保持当前身份。
- `container::ContainerRuntimes`：实现 `ContainerInventory`、`resolve`、`published_on`、`verified_host_pid`、`enrich` 与 `ContainerHealthcheckProbe`。七种运行时（Docker、Podman、Nerdctl、Crictl、Incus、LXD、Lxc）各自独立探测与失败，按 runtime + id 去重；command/Compose 匹配键只存在于私有且不可序列化的 `ListedContainer`，公共列表不外泄；nerdctl 的稳定键/显示名为 containerd，crictl 为 k8s；容器 ID 交 CLI 前先校验格式；Docker 发布端口回退使用固定 argv。运行时报告的 PID 只是候选，须经 `ContainerProcessVerifier` 验证（Linux 以 `/proc/PID/cgroup` 二次确认；Windows 恒 false，Docker Desktop VM PID 不可映射）。
- `linux::LinuxPlatform`（仅 `target_os = "linux"`）：实现 ProcessInventory、ProcessDetailsProvider、NetworkInventory、FileInventory、ProcessFileLocks、SourceEvidenceProvider、ContainerProcessVerifier 与 ProcessController 八个端口。生产 PID 枚举用 sysinfo、真实墙钟建立排除基线，可选详情字段失败保留数据并逐项诊断；`FileInventory::list` 合并可见 `/proc/PID/fd` 与真实锁，`holders(path)` 以路径（含可解析时的 canonical 路径）筛选，权限/读取问题转为有界诊断；systemd D-Bus 有方法超时与 single-flight；健康标签按 parity 计算（Z/T/HighCpu/HighMem 阈值）。所有 `/proc` 读取经可注入根目录的 `ProcFs`，注入平台始终禁用控制副作用。
- `linux` 进程控制（`src/linux/controller.rs`）：先 `pidfd_open` 固定进程对象，再重读启动时间与 executable 身份并比对（不一致返回 `ProcessChanged`）；信号经 `pidfd_send_signal`——Terminate=SIGTERM、Kill 与 KillTree 目标=SIGKILL、Pause=SIGSTOP、Resume=SIGCONT；Renice 先 signal 0 再按 PID 调 `setpriority`，两次 syscall 之间仍有已披露的极窄 PID 复用 TOCTOU。`KillTree` 以当前快照经 core `collect_descendants` 求后代，含自身整体拒绝，逐个 `pidfd_open` + 身份比对后 SIGKILL（`NotFound` 视为已清杀），权限等错误聚合上抛。身份不可验证、自身 PID、注入平台与系统调用错误返回结构化 `InspectError`，不降级到裸 `kill` 或提权。
- `windows::WindowsPlatform`（仅 `target_os = "windows"`）：实现 ProcessInventory、ProcessDetailsProvider、NetworkInventory、SourceEvidenceProvider、ProcessController、FileInventory、ProcessFileLocks、ContainerProcessVerifier 八个端口。采集侧为 sysinfo 基线 + ToolHelp32 快照补名、IP Helper 端口表（IPv4/IPv6 TCP/UDP + PID）、PEB/PEB32 有界远程读取（UTF-16 环境块解码）、SCM 来源证据与资源字段（memory、`GetProcessTimes`、`GlobalMemoryStatusEx`），全部经 `windows-sys` 安全包装（`ffi/`、`ffi_scm`）。
- `windows` 进程控制与定位（`src/windows/controller.rs`）：`capability()` 恒 Supported；`action_capability` 逐动作——Terminate/Kill/KillTree 为 Supported，Pause/Resume/Renice 为 `Unsupported(KILL_ONLY_REASON)`。Terminate/Kill 走 `OpenProcess` → `GetProcessTimes` 创建时间比对 → `TerminateProcess`（整秒对齐比较，防 PID 复用；拒绝控制自身 PID；快照缺 start_time 返回 `ProcessChanged`）。KillTree 先杀目标再对 sysinfo 实时快照的 core 后代逐个执行同一套比对+终止，后代含自身则整体拒绝，`OpenProcess` 失败按 access denied → `PermissionDenied`、gone/invalid → 视为已退出、其余 → `Unsupported` 三分。`reveal_capability` 恒 Supported；`reveal_executable` 路径实时优先 `QueryFullProcessImageNameW`、回退 sysinfo、再回退 `identity.executable()` 快照路径，两者皆无返回 `NotFound`，随后经 `ShellExecuteW` 委托 `explorer.exe /select`（包装在 `ffi/shell.rs`，`/select` 参数双引号包裹），失败按 `ShellExecuteW` 返回码分流为 `NotFound` / `PermissionDenied` / `ExternalTool`（`reveal.rs` 纯分类，`<= 32` 判失败），不把运行期失败说成平台能力缺失。
- Windows 不支持项（`src/windows/limits.rs` + `src/windows/unsupported.rs`）：FileInventory/ProcessFileLocks 返回带稳定原因键的 Unsupported 失败结果，ContainerProcessVerifier 恒 false；原因键本体在 `unsupported.rs`（`FILE_LOCKS_REASON`、`KILL_ONLY_REASON`）。

## 关键依赖与配置

- `runquiry-core.workspace = true`；`sysinfo 0.31.4`（进程基线）、`serde 1.0.229` / `serde_json 1.0.151`（容器 JSON 与解析）、`zbus 5.19.0`（systemd D-Bus，blocking，仅 Linux 语义）；`cfg(unix)`：`libc 0.2.189`（进程组终止）；`cfg(target_os = "windows")`：`windows-sys 0.61.2`，启用 12 个 feature（Foundation、Threading、ProcessStatus、ToolHelp、Debug、Services、SystemInformation、IpHelper、WinSock、Wdk_System_Threading、UI_Shell、UI_WindowsAndMessaging）。
- 无 dev-dependencies、无自定义 features；crate 级 `#![allow(clippy::multiple_crate_versions)]`（锁定树内 syn 2/3 双版本），另有若干文件级 `redundant_pub_crate` 豁免（容器解析模块）。
- 依赖变更（含 windows-sys feature）由依赖守门人集中执行并确认锁文件零新增包；GPUI source 不参与本 crate。

## 测试与质量

静态计数（`#[test]` 属性匹配，非运行结果；部分内联模块被多个测试目标经 `#[path]` 复用，会重复计入）：

| 测试目标 | 数量 | 门控 |
|---|---|---|
| lib 内联（`src/`） | 62 | Windows 目标 57 / Linux 目标 11（含 `original_user` 等 `cfg(unix)` 与 Windows 真机活测试） |
| `tests/command_runner.rs` | 10 | `cfg(unix)` |
| `tests/container_crictl.rs` / `container_lxc.rs` / `container_lxd_like.rs` / `container_port_fallback.rs` | 6 / 3 / 4 / 3 | `cfg(unix)` |
| `tests/container_docker_like.rs` / `container_inventory.rs` | 11 / 9 | 跨平台 |
| `tests/fake_backends.rs` | 10 | 跨平台（`tests/support/` 提供假后端） |
| `tests/linux_adapters.rs` | 30 | `target_os = "linux"` |
| `tests/process_controller.rs` | 6 | `target_os = "linux"`（真实自建进程、身份拒绝、注入平台无副作用） |
| `tests/windows_ip_table.rs` / `windows_peb.rs` / `windows_scm.rs` / `windows_utf16.rs` | 14 / 23 / 19 / 12 | 纯解析，经 `#[path]` 引入 src 模块，Linux 也可直接运行 |
| `tests/windows_reveal.rs` / `windows_unsupported.rs` / `windows_winerror.rs` | 4 / 4 / 5 | 纯解析，同上 |

- 命令：`cargo test -p runquiry-platform --locked`（全量）、`--test <目标名>` 定向，如 `--test process_controller`、`--test windows_reveal`。
- Linux 采集测试用合成 /proc tempdir 树（可注入根目录），不读真实 /proc、不依赖 root、不 sleep；容器测试用 tempdir 假 CLI 经生产 `StdCommandRunner` 驱动。
- Windows 真机活测试位于 `src/windows/controller.rs`（单杀、杀树）与 `src/windows/process_list.rs`（ToolHelp 快照），仅在 Windows 目标编译运行。
- 测试代码同样受 `unwrap_used`/`expect_used = deny`（无 clippy.toml 豁免）：新测试一律 `Result + ?` / `unwrap_or` / `assert!`。
- 真实 QA 示例（`cargo run -p runquiry-platform --example <名> --locked`）：`linux_qa`（普通用户真实采集，环境变量只报计数）、`container_qa`（真实只读 list/verified_host_pid/enrich，daemon 不可用或 CLI 缺失时如实报 Partial）、`process_controller_qa`（只控制自建并清理的 `sleep` 子进程）、`windows_qa`（Windows 只读采集与 Unsupported 边界，敏感值只报计数）。

## 常见问题

- 新增依赖必须经依赖守门人集中修改 manifest 并确认 Cargo.lock 零新增包（见模块 01 计划的所有权边界）。
- 单文件纯代码 ≤250 行红线：超限按职责拆分（procfs、dockerlike 等均已拆为子模块）。
- `windows/limits.rs` 文件名沿用早期「能力上限/stub」称呼，实际内容是 File Locks 与容器归属的 Unsupported 建模，原因键在 `unsupported.rs`。

## 相关文件清单

- `crates/runquiry-platform/Cargo.toml` — crate manifest（含 windows-sys feature 清单，守门人维护）
- `crates/runquiry-platform/src/lib.rs` — 模块声明、cfg 门禁与 crate 级 clippy 豁免
- `crates/runquiry-platform/src/command/` — `runner`/`process`/`output`/`original_user`：执行、进程组回收、有界采集、sudo 原用户恢复
- `crates/runquiry-platform/src/container/` — 七运行时适配器与汇总（`mod`/`inventory`/`runtime`/`parse`/`dockerlike{,/wire,/port}`/`crictl`/`lxdlike`/`lxc`）
- `crates/runquiry-platform/src/linux/` — Linux 端口实现（`process`/`details`/`summary`/`network`/`open_files`/`locks`/`fdscan`/`file_diagnostics`/`source{,/systemd}`/`capabilities`/`container`/`procfs/*`）
- `crates/runquiry-platform/src/linux/controller.rs` — 身份复核、pidfd 信号、KillTree 与 renice 控制
- `crates/runquiry-platform/src/windows/` — Windows 采集（`process_list`/`details`/`network`/`source` 与 `ip_table/*`、`peb/*`、`peb_reader`、`utf16`、`scm_parse`、`winerror` 纯解析层）
- `crates/runquiry-platform/src/windows/controller.rs` — 关闭类进程控制 + `reveal_*`（含真机活测试）
- `crates/runquiry-platform/src/windows/reveal.rs`、`src/windows/ffi/shell.rs` — `/select` 参数与 `ShellExecute` 返回码分类、`ShellExecuteW` 安全包装
- `crates/runquiry-platform/src/windows/limits.rs`、`src/windows/unsupported.rs` — Unsupported 建模与稳定原因键
- `crates/runquiry-platform/src/windows/ffi/`、`src/windows/ffi_scm.rs` — 全部 FFI 安全包装
- `crates/runquiry-platform/tests/` — 命令执行、容器、Linux 采集与控制、Windows 纯解析套件与假后端（超长套件按同名子目录拆分）
- `crates/runquiry-platform/examples/` — `linux_qa`、`container_qa`、`process_controller_qa`、`windows_qa`
- `docs/witr-parity.md` — 采集、运行时与进程操作的行为契约
- `.omo/plans/runquiry-gpui-desktop/03-container-runtime.md`、`04-linux-platform.md`、`07-windows-platform.md` — 任务定义
