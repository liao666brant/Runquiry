# Runquiry Platform 平台层

[crates](../) / runquiry-platform

## 模块职责

平台采集与控制层：Linux/macOS/Windows 采集器、容器运行时集成、进程控制（terminate/kill/pause/resume/renice）与外部命令执行。B2 已落地 Linux 只读采集（`src/linux/`），B3 已落地受限命令执行器与七种容器运行时（`src/command/`、`src/container/`）；macOS/Windows 采集与进程控制（Unix 控制、三平台差异）属后续任务（模块 04/06/07）。

约束：只依赖 runquiry-core，为其端口（trait）提供实现；UI 不得绕过本层直接读取 /proc、调用 Win32 API 或运行 lsof/容器 CLI。全部外部命令（容器 CLI、QA 假 CLI）只能经 `StdCommandRunner`，绝不经过 shell。

## 入口与启动

无独立入口。库 crate，`src/lib.rs` 导出 `command`、`container` 与 cfg(linux) 的 `linux` 模块。

## 对外接口

- `command::StdCommandRunner`：`CommandRunner` 生产实现 + `run_classified`（`CommandFailure::Spawn/Timeout` 供调用方映射诊断）。程序名+独立 argv、stdout/stderr 并发读取分别限 8MiB、超时/超限对进程组 SIGKILL（`process_group(0)` + `kill(-pgid)`）并 wait 回收，不留孤儿/僵尸。
- `container::ContainerRuntimes`：`ContainerInventory` 实现 + `list_detailed()`（返回 `ListedContainer`：快照 + Compose 临时匹配键，**不进** `ContainerSummary`/UI）、`host_pid`、`enrich`（`ContainerEnrichment`）、`ContainerHealthcheckProbe`（仅 docker/podman 可判定，inspect `{{json .Config.Healthcheck}}`）；`RuntimeBinaries` 注入 CLI 名/路径（测试假 CLI）。七运行时（docker/podman/nerdctl→containerd 显示/crictl→k8s/incus/lxd/lxc）各自独立 available/list/host_pid/enrich；单运行时失败只追加 DiagnosticIssue；按 runtime+id 去重。
- `linux::LinuxPlatform`（仅 `target_os = "linux"`）：实现 ProcessInventory、ProcessDetailsProvider、NetworkInventory、FileInventory、ProcessFileLocks、SourceEvidenceProvider 六个只读端口；`new()` 生产 `/proc`，`with_injected(proc_root, systemd_run_dir, own_pid)` 注入合成树。自身排除：构造期 PID 基准快照 + 迟到后代（PPID 链或启动时间窗）；健康标签按 parity 计算（Z/T/HighCpu >2h/HighMem >1GiB）。平台侧不做来源类型判定（core `detect_source` 职责）。
- 未实现：ProcessController 真实实现（B7）、macOS/Windows 采集（C1/C2）。

## 关键依赖与配置

- 依赖：runquiry-core、sysinfo 0.31.4（进程基线）、serde 1.0.229 / serde_json 1.0.151（容器 JSON 解析）、zbus 5.19.0（systemd D-Bus，blocking，仅 Linux 语义）、libc 0.2.189（cfg(unix)，仅 kill(2) 进程组终止）。全部为 Cargo.lock 既有包（GPUI 传递依赖），零新增包；manifest 变更由主 Agent（依赖守门）执行。
- Linux 构建系统依赖（pkg-config、fontconfig 等）见根 AGENTS.md。

## 测试与质量

- `cargo test -p runquiry-platform --locked`：75 个测试（lib 单测 4 + tests/command_runner.rs 8 + tests/container_* 33 + tests/fake_backends.rs 10 + tests/linux_adapters.rs 20）。定向过滤器：`command`（9）、`container`（37）、`linux`（20）。
- linux 采集测试用合成 /proc tempdir 树（可注入根目录），不读真实 /proc、不依赖 root、不 sleep；容器测试用 tempdir 假 CLI 经生产 `StdCommandRunner` 驱动。
- 真实 QA 示例：`cargo run -p runquiry-platform --example linux_qa --locked`（普通用户真实采集，环境变量只报计数）；`--example container_qa`（真实 Docker 只读 list/host_pid/enrich，缺失 CLI 如实报告）。

## 常见问题

- 新增依赖必须经依赖守门人集中修改 manifest 并确认 Cargo.lock 零新增包（见模块 01 计划的所有权边界）。
- 单文件纯代码 ≤250 行红线：超限按职责拆分（procfs 与 dockerlike 均已拆为子模块）。
- 平台 clippy 豁免 `multiple_crate_versions`（锁定树既有 syn 2/3 双版本，见 src/lib.rs 注释）。

## 相关文件清单

- `crates/runquiry-platform/Cargo.toml` — crate manifest（守门人维护）
- `crates/runquiry-platform/src/lib.rs` — 模块声明（linux cfg 隔离）
- `crates/runquiry-platform/src/command/` — StdCommandRunner 与失败分类
- `crates/runquiry-platform/src/container/` — 七运行时适配器（runtime/crictl/dockerlike{,/wire}/lxdlike/lxc/parse）
- `crates/runquiry-platform/src/linux/` — Linux 只读采集（procfs{,/process_files,netparse,locktable}/process/details/fdscan/network/locks/source/capabilities）
- `crates/runquiry-platform/tests/` — command_runner、container_*、linux_adapters、fake_backends
- `crates/runquiry-platform/examples/` — linux_qa、container_qa（真实 QA）
- `docs/witr-parity.md` — 采集与运行时行为契约
- `.omo/plans/runquiry-gpui-desktop/03-container-runtime.md`、`04-linux-platform.md` — B3/B2 任务定义

## 变更记录

- 2026-09-02：初次索引。骨架状态，仅有 manifest 与 lib.rs 占位。
- 2026-09-03：Batch 2 A5——新增 tests/support/（Scenario 失败注入、FakePlatform 假实现）与 tests/fake_backends.rs。
- 2026-09-04：Batch 3——B2 Linux 只读适配器（`src/linux/`，20 测试 + linux_qa 示例）；B3 StdCommandRunner 与七容器运行时（`src/command/`、`src/container/`，41 测试 + container_qa 示例）；新增直接依赖 sysinfo/serde/serde_json/zbus/libc（全部锁内既有包，零新增）；lib.rs 声明 linux cfg 隔离与 crate 级 clippy 豁免；dockerlike/procfs 按 250 行红线拆分子模块；CommandRunner 进程组终止修复超时孙进程遗留。评审修复：LinuxPlatform 实现 `ProcessFileLocks`（/proc/locks 按 PID 过滤）、健康标签 HighCpu/HighMem（parity 阈值）、收养后代时间窗排除、用户表每轮 list() 复用；ContainerRuntimes 实现 `ContainerHealthcheckProbe`（docker/podman）。测试增至 75 个。