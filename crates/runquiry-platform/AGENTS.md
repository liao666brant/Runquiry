# Runquiry Platform 平台层

[crates](../) / runquiry-platform

## 模块职责

平台采集与控制层：Linux/macOS/Windows 采集器、容器运行时集成、进程控制（terminate/kill/pause/resume/renice）与外部命令执行。B2 已落地 Linux 只读采集（`src/linux/`），B3 已落地受限命令执行器与七种容器运行时（`src/command/`、`src/container/`）；Batch 4A P1 以 `/proc/PID/fd` 与 `/proc/locks` 提供文件/锁清单和路径持有者查询；macOS/Windows 采集与进程控制（Unix 控制、三平台差异）属后续任务（模块 04/06/07）。

约束：只依赖 runquiry-core，为其端口（trait）提供实现；UI 不得绕过本层直接读取 /proc、调用 Win32 API 或运行 lsof/容器 CLI。全部外部命令（容器 CLI、QA 假 CLI）只能经 `StdCommandRunner`，绝不经过 shell。

## 入口与启动

无独立入口。库 crate，`src/lib.rs` 导出 `command`、`container` 与 cfg(linux) 的 `linux` 模块。

## 对外接口

- `command::StdCommandRunner`：`CommandRunner` 生产实现；`CommandFailure` 区分 Spawn/Timeout/OutputLimit/Cancelled。程序名 + 独立 argv；stdout/stderr 各限 8MiB，超限、取消与超时均终止进程组并 wait，读取错误不伪装成功。`CancellationToken` 支持运行中取消；sudo 下 Podman/nerdctl 经校验后恢复原用户 UID/GID 与 rootless 环境，Docker 保持当前身份。
- `container::ContainerRuntimes`：实现 `ContainerInventory`、`resolve`、`published_on`、`verified_host_pid`、`enrich` 与 `ContainerHealthcheckProbe`。command/Compose 匹配键只存在于私有且不可序列化的 `ListedContainer`，公共列表不外泄；nerdctl 的稳定键/显示名为 containerd，crictl 为 k8s。Docker 发布端口回退使用固定 argv；运行时 PID 只是候选，Linux `ContainerProcessVerifier` 必须以 `/proc/PID/cgroup` 再验证。七运行时独立失败，按 runtime+id 去重。
- `linux::LinuxPlatform`（仅 `target_os = "linux"`）：实现 ProcessInventory、ProcessDetailsProvider、NetworkInventory、FileInventory、ProcessFileLocks、SourceEvidenceProvider、ContainerProcessVerifier 七个只读边界。`FileInventory::list` 合并可见 `/proc/PID/fd` 与真实锁，`holders(path)` 以路径（含可解析时的 canonical 路径）筛选；权限/读取问题转为有界诊断。`new()` 以 sysinfo 枚举生产 PID、真实墙钟建立排除基线，`with_injected` 使用合成 ProcFs。可选详情字段失败保留数据并逐项诊断；systemd D-Bus 有 2s 方法超时与 single-flight。健康标签按 parity 计算（Z/T/HighCpu >2h/HighMem >1GiB）。
- 未实现：ProcessController 真实实现（B7）、macOS/Windows 采集（C1/C2）。

## 关键依赖与配置

- 依赖：runquiry-core、sysinfo 0.31.4（进程基线）、serde 1.0.229 / serde_json 1.0.151（容器 JSON 解析）、zbus 5.19.0（systemd D-Bus，blocking，仅 Linux 语义）、libc 0.2.189（cfg(unix)，仅 kill(2) 进程组终止）。全部为 Cargo.lock 既有包（GPUI 传递依赖），零新增包；manifest 变更由主 Agent（依赖守门）执行。
- Linux 构建系统依赖（pkg-config、fontconfig 等）见根 AGENTS.md。

## 测试与质量

- `cargo test -p runquiry-platform --locked`：97 个测试。命令双流竞争回归另连续执行 10 次通过；`cargo clippy -p runquiry-core -p runquiry-platform --all-targets --locked -- -D warnings` 零警告。
- linux 采集测试用合成 /proc tempdir 树（可注入根目录），不读真实 /proc、不依赖 root、不 sleep；容器测试用 tempdir 假 CLI 经生产 `StdCommandRunner` 驱动。
- 真实 QA 示例：`cargo run -p runquiry-platform --example linux_qa --locked`（普通用户真实采集，环境变量只报计数）；`--example container_qa`（真实只读 list/verified_host_pid/enrich；daemon 不可用或 CLI 缺失时如实报告 Partial）。

## 常见问题

- 新增依赖必须经依赖守门人集中修改 manifest 并确认 Cargo.lock 零新增包（见模块 01 计划的所有权边界）。
- 单文件纯代码 ≤250 行红线：超限按职责拆分（procfs 与 dockerlike 均已拆为子模块）。
- 平台 clippy 豁免 `multiple_crate_versions`（锁定树既有 syn 2/3 双版本，见 src/lib.rs 注释）。

## 相关文件清单

- `crates/runquiry-platform/Cargo.toml` — crate manifest（守门人维护）
- `crates/runquiry-platform/src/lib.rs` — 模块声明（linux cfg 隔离）
- `crates/runquiry-platform/src/command/` — runner/process/output/original_user：执行、回收、采集、原用户恢复
- `crates/runquiry-platform/src/container/` — 七运行时适配器与汇总（inventory/runtime/crictl/dockerlike{,/wire,/port}/lxdlike/lxc/parse）
- `crates/runquiry-platform/src/linux/` — Linux 只读采集（procfs/process/summary/details/fdscan/network/locks/open_files/file_diagnostics/source/capabilities/container）
- `crates/runquiry-platform/tests/` — command_runner、container_*、linux_adapters、fake_backends（超长套件按同名子目录拆分）
- `crates/runquiry-platform/examples/` — linux_qa、container_qa（真实 QA）
- `docs/witr-parity.md` — 采集与运行时行为契约
- `.omo/plans/runquiry-gpui-desktop/03-container-runtime.md`、`04-linux-platform.md` — B3/B2 任务定义

## 变更记录

- 2026-09-02：初次索引。骨架状态，仅有 manifest 与 lib.rs 占位。
- 2026-09-03：Batch 2 A5——新增 tests/support/（Scenario 失败注入、FakePlatform 假实现）与 tests/fake_backends.rs。
- 2026-09-04：Batch 3——B2 Linux 只读适配器（`src/linux/`，20 测试 + linux_qa 示例）；B3 StdCommandRunner 与七容器运行时（`src/command/`、`src/container/`，41 测试 + container_qa 示例）；新增直接依赖 sysinfo/serde/serde_json/zbus/libc（全部锁内既有包，零新增）；lib.rs 声明 linux cfg 隔离与 crate 级 clippy 豁免；dockerlike/procfs 按 250 行红线拆分子模块；CommandRunner 进程组终止修复超时孙进程遗留。评审修复：LinuxPlatform 实现 `ProcessFileLocks`（/proc/locks 按 PID 过滤）、健康标签 HighCpu/HighMem（parity 阈值）、收养后代时间窗排除、用户表每轮 list() 复用；ContainerRuntimes 实现 `ContainerHealthcheckProbe`（docker/podman）。测试增至 75 个。
- 2026-09-04：阻断项修复——CommandRunner 增加取消、超限硬失败、读取错误传播与稳定双流回归；rootless CLI 恢复 sudo 原用户。容器解析保留 command/Compose 私有临时键，补发布端口回退，nerdctl 键改为 containerd，host PID 通过 Linux cgroup 验证。Linux 生产 PID 枚举改回 sysinfo，构造墙钟生效，详情字段错误进入 Inspection，systemd 富化设方法超时和 single-flight。拆分超长测试与 runner 职责文件，测试增至 90 个。
- 2026-09-04（未提交工作区）：Batch 4A P1——新增 `linux/open_files.rs` 与 `file_diagnostics.rs`，`LinuxPlatform` 为 File Locks 工作区提供真实打开文件/锁清单及路径持有者查询，合成 ProcFs 回归覆盖 PID 0、权限与锁/FD 合并。platform 测试增至 97 个。
