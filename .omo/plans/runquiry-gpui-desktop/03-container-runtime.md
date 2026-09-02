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

- [ ] **B3 容器运行时与安全命令执行器**
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
