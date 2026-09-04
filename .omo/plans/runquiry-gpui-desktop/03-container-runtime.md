# 模块 03：外部命令与容器运行时

上级方案：[Runquiry 桌面化实施计划](../runquiry-gpui-desktop.md)

## 模块目标

提供受限、可测试的外部命令边界，并通过独立适配器汇总主机上实际可用的容器运行时。一个运行时失败不能阻断其他运行时或应用页面。

## 所有权与边界

允许写入：

- crates/runquiry-platform/src/command/
- crates/runquiry-platform/src/container/
- crates/runquiry-platform/tests/command_*
- crates/runquiry-platform/tests/container_*

不得写入：

- OS 进程、端口、文件锁模块。
- UI 页面、根 Cargo 配置、Cargo.lock。
- shell 脚本拼接或 shell 字符串执行路径。

## TODOs

- [x] **B3 容器运行时与安全命令执行器**
  - 依赖：A3、A5。
  - CommandRunner 输入必须是程序路径、argv、超时和输出上限，禁止 shell。
  - 实现 500ms 可用性探测、3s 列表调用和 5s 详情调用。
  - stdout/stderr 分别限制 8MiB；超限终止子进程并返回 ExternalTool。
  - 支持 Docker、Podman、nerdctl、crictl、Incus、LXC、LXD。
  - 每个运行时独立实现 Available、List、HostPid、Enrich。
  - 只解析 JSON 或稳定机器格式。
  - 按 runtime + container id 去重；查询匹配覆盖名称、镜像、命令和 Compose project/service。
  - 主机 PID 不存在或无法验证时保留容器部分结果，不伪造进程详情。
  - 子进程退出、超时或页面取消后不得遗留后台进程。
  - 验证：
    - cargo test -p runquiry-platform command --locked
    - cargo test -p runquiry-platform container --locked
    - 使用假可执行文件验证 argv 边界、超时、终止和输出限制
    - 在安装 Docker 的 Linux 主机执行一次真实列表和详情读取
  - 完成证据：运行时能力表、失败隔离测试、进程清理记录、真实 Docker 只读结果。
  - 实施记录（2026-09-04）：
    - `command/`：`StdCommandRunner` 实现 `CommandRunner`，分类失败覆盖 Spawn/Timeout/OutputLimit/Cancelled；只接受程序名与独立 argv（绝不 shell）。stdout/stderr 由独立读取器并发采集且各限 8MiB，任一流超限、调用方取消或超时时立即终止进程组并 wait 回收，绝不把截断内容作为成功结果；读取器 I/O/通道失败也显式返回 ExternalTool。Unix 以 `process_group(0)` + `kill(-pgid)` 清理派生孙进程；ETXTBSY 仅做有界短退避。Podman/nerdctl 在 sudo 下通过已校验的 SUDO_UID/SUDO_GID/SUDO_USER 恢复原用户 UID、GID、HOME、USER、LOGNAME 与 XDG_RUNTIME_DIR，Docker 保持当前身份。
    - `container/`：`ContainerRuntimes` 实现 `ContainerInventory`；私有、不可序列化的 `ListedContainer` 仅在 `resolve` 路径携带 command/Compose project/service 五字段匹配键，公共 `list` 只返回 `ContainerSummary`。七运行时各自实现 available/list/host_pid/enrich：docker（`ps --no-trunc --format {{json .}}` 逐行 JSON）、podman/nerdctl（`--format json` 数组）、crictl（`ps -o json` + inspect 双形态解析）、incus/lxd（`list --format json` 共享 REST 解析；LXD 需 lxc+lxd 双二进制）、lxc（`lxc-ls --fancy --format json` + `lxc-info -n <id> -p -H`）；nerdctl 的稳定 `ContainerKey.runtime` 为 `containerd`。`inspect --format {{json .State}}` 取 host PID 候选与 StartedAt；Linux `ContainerProcessVerifier` 再读取候选 PID 的 cgroup，ID 缺失或不匹配则返回 None。Docker 发布端口回退使用固定 argv `ps --filter publish=<port>`。serde 专属结构解析，容器 ID 进 CLI 前校验；单运行时失败只追加 DiagnosticIssue；按 runtime+id 去重。FreeBSD jail 未实现。
    - 评审修复（2026-09-04）：实现 `ContainerHealthcheckProbe`（parity `ContainerHealthcheckStatus`：仅 docker/podman；`inspect --format {{json .Config.Healthcheck}}`，Test 非空 → Present，null/空 → Absent，inspect 失败 → None 即不触发告警，与 witr 空串语义一致）；此前管线该端口无生产实现，「容器无健康检查」告警不可达。
    - 测试：`command_runner` 10 个、六个 `container_*` 行为套件 36 个，另有 command/container 单元测试 4 个；全部经生产 CommandRunner + tempdir 假 CLI。覆盖 argv 逐字透传、CLI 缺失、非零退出含部分 stdout、超时、取消、stdout/stderr/双流竞争超限失败与回收、读取器错误、损坏 JSON/字段类型错、命令与 Compose 五字段解析、nerdctl 稳定键、Docker 发布端口回退、rootless 身份恢复、跨运行时去重以及 host PID cgroup 拒绝路径。原不稳定的双流竞争测试连续执行 10 次通过。
    - 依赖：新增直接依赖 sysinfo 0.31.4 / serde 1.0.229 / serde_json 1.0.151 / zbus 5.19.0（B2 共用）与 libc 0.2.189（仅 kill(2) 进程组终止，cfg(unix)）——全部为锁内既有包，Cargo.lock 仅 runquiry-platform 条目依赖列表更新，零新增包、零版本变化；GPUI f66ed399 / gpui-component 91217366 未漂移。
    - 真实 QA：较早的本机 WSL2 Docker 29.5.1 验证曾读取 7 个运行中容器并完成健康、host PID 与 StartedAt 富化。2026-09-04 本轮复验环境仅检测到 docker CLI，但 daemon/list 返回退出码 1；其余六类 CLI 缺失，`container_qa` 如实返回 Partial 与逐运行时 external_tool_failed，未伪造容器数据。正向 Docker daemon 场景仍以较早证据为准，本轮环境边界不改写为通过。
    - 已知限制：Incus/LXD/LXC 的网络与挂载富化未实现（返回空富集，witr enrich 超出本轮字段集）；容器快照不含已停止容器（parity：witr `ps` 无 -a）。

## 必测失败模式

- CLI 不在 PATH。
- CLI 返回非零状态且 stdout 含部分合法数据。
- 命令挂起。
- stdout 或 stderr 超限。
- JSON 截断、字段缺失或类型错误。
- 两个运行时返回相同短 ID。
- 容器已停止或主机 PID 为零。
- HostPid 指向与容器不一致的进程。

## 模块退出条件

- 无 shell 注入面。
- 所有运行时通过同一 contract suite。
- 任一运行时失败时其他运行时结果仍可返回。
- 测试和真实 QA 启动的全部子进程均已清理。

## 交接格式

- 支持的运行时与对应机器格式。
- CommandRunner 的超时、上限和终止语义。
- 各运行时的可用、部分可用和失败证据。
