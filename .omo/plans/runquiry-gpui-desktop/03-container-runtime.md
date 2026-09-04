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
    - `command/runner.rs`：`StdCommandRunner`（CommandRunner 生产实现）+ `run_classified`（CommandFailure::Spawn/Timeout 供运行时映射诊断）。只接受程序名+独立 argv（绝不 shell）；stdout/stderr 各一线程并发读取、分别限 8MiB、超限截断并继续排空；超时/超限先对进程组 SIGKILL（spawn 时 `process_group(0)`，kill(-pgid) 终止子进程派生的孙进程）再 kill+wait 回收，不留孤儿；ETXTBSY 短退避重试（WSL2 实测瞬态）。
    - `container/`：`ContainerRuntimes`（ContainerInventory 实现，`list_detailed` 含 Compose 临时键、`list` 丢弃临时键）+ 七运行时各自独立实现 available/list/host_pid/enrich：docker（`ps --no-trunc --format {{json .}}` 逐行 JSON）、podman/nerdctl（`--format json` 数组）、crictl（`ps -o json` + inspect 双形态解析）、incus/lxd（`list --format json` 共享 REST 解析；LXD 需 lxc+lxd 双二进制）、lxc（`lxc-ls --fancy --format json` + `lxc-info -n <id> -p -H`）；`inspect --format {{json .State}}` 取 host PID 与 StartedAt（enrich）。serde 专属结构解析，`serde_json::Value` 不越过解析边界；容器 ID 进 CLI 前经 is_valid_container_id 校验；单运行时失败只追加 DiagnosticIssue；按 runtime+id 去重，短 ID 不跨运行时合并；host PID 缺失/零/负 → None 不伪造。FreeBSD jail 未实现。
    - 评审修复（2026-09-04）：实现 `ContainerHealthcheckProbe`（parity `ContainerHealthcheckStatus`：仅 docker/podman；`inspect --format {{json .Config.Healthcheck}}`，Test 非空 → Present，null/空 → Absent，inspect 失败 → None 即不触发告警，与 witr 空串语义一致）；此前管线该端口无生产实现，「容器无健康检查」告警不可达。
    - 测试（command_runner 8 + container_* 32 = 40，全部经生产 CommandRunner + tempdir 假 CLI）：argv 逐字透传（空格/引号/换行/- 开头）、CLI 缺失、非零退出含部分 stdout、挂起超时、stdout/stderr/双流超限截断+回收、截断 JSON/字段类型错 → ParseFailed、双运行时同短 ID 不合并、单运行时失败隔离、host PID 零/缺失、进程组清理（测试后 0 孤儿进程实测）。
    - 依赖：新增直接依赖 sysinfo 0.31.4 / serde 1.0.229 / serde_json 1.0.151 / zbus 5.19.0（B2 共用）与 libc 0.2.189（仅 kill(2) 进程组终止，cfg(unix)）——全部为锁内既有包，Cargo.lock 仅 runquiry-platform 条目依赖列表更新，零新增包、零版本变化；GPUI f66ed399 / gpui-component 91217366 未漂移。
    - 真实 QA（本机 WSL2，docker server 29.5.1）：生产 ContainerInventory 真实只读 list（7 个运行中容器，长 ID、`(healthy)` 健康提取正确）+ host_pid（返回真实主机 PID）+ enrich（StartedAt）；podman/containerd/k8s/incus/lxd/lxc 缺失如实报 Partial + external_tool_failed 诊断，未伪造成功。
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
