# 模块 06：macOS 平台

上级方案：[Runquiry 桌面化实施计划](../runquiry-gpui-desktop.md)

## 模块目标

在 Linux 纵向闭环稳定后补齐 macOS 15+ 的进程、网络、文件、launchd、容器关联和 Unix 进程操作。

## 所有权与边界

允许写入：

- crates/runquiry-platform/src/macos/
- crates/runquiry-platform/tests/macos_*
- macOS fixture 与 QA 证据

不得写入：

- core 领域规则、UI、Windows/Linux 适配器。
- 容器运行时通用实现。
- 根 Cargo 配置和 Cargo.lock。

若需要 macOS 专用依赖，由 A1 负责人集中修改 manifest 和锁文件。

## TODOs

- [ ] **C1 macOS 适配器**
  - 进度（2026-09-07，Batch 6）：`src/macos/` 已落地（process/details/network/files/source/controller/container/libproc + lsof/launchctl/plist/identity 纯解析），libproc 绑定为 libc 手写（零新增依赖）；lsof -F 机器格式解析覆盖空格/中文/换行路径与非零退出抢救；进程控制为身份重读 + kill(2)/setpriority 并披露 PID 复用窗口。`tests/macos_*` 三个纯解析测试文件 + `examples/macos_qa.rs` 双 main。独立 FFI 审查阻断项（ProcTaskInfo 线程字段宽度、rusage diskio 偏移 128/136）已修复；静态评审后已补齐：测试 unwrap 基线清理、app 侧名称解析 launchd 回退接线（`launchd_service_pid`）。**未完成**：任何编译/测试均未运行（本机 WSL 编译卡死，用户叫停）；macOS 实机验证、lsof 超时/部分失败实机场景、动作后 OS 状态核对、临时进程清理回执全部未验收；rusage 偏移仍需在 macOS 与 SDK 头文件比对。
  - 依赖：B8。
  - 使用 sysinfo 提供基线进程列表，使用 libproc 补齐 exe、cwd、祖先、资源和文件信息。
  - lsof 调用使用 -F 机器格式，提供端口、Socket、打开文件和 best-effort 文件锁。
  - 解析 launchd/plist 并结合祖先链生成启动来源证据。
  - 复用模块 03 的容器运行时，不复制 Docker/Podman 逻辑。
  - 复用 Unix ProcessController，并实现 macOS 身份校验和错误映射。
  - 权限不足或 lsof 部分失败时保留可读取数据。
  - 所有 FFI 收敛到安全包装模块；每个 unsafe 块只包含一个必要操作并写明安全条件。
  - 验证：
    - cargo test -p runquiry-platform macos --locked
    - macOS 15+ 实机读取进程、端口、文件、launchd 和容器
    - 对任务自建临时进程验证 pause、resume、terminate、kill
    - 普通用户验证部分权限失败
  - 完成证据：Apple Silicon 实机记录、Intel fixture/runner 结果、临时进程清理回执。

## 必测场景

- lsof 不存在、超时、非零退出但包含部分 stdout。
- 进程在 libproc 查询期间退出。
- launchd plist 缺失、不可读或字段不完整。
- 文件名和路径包含空格、中文与换行转义。
- Docker Desktop 未启动、已启动但容器停止、容器主机 PID 不可见。
- renice 权限不足。

## 模块退出条件

- macOS 的四工作区和五类调查均经过实机验证，平台不支持项有明确状态。
- 无复制的容器运行时或核心告警逻辑。
- QA 创建的进程和容器全部清理。

## 交接格式

- macOS 能力矩阵和 best-effort 边界。
- 实机硬件、系统版本和测试场景。
- 自动测试、操作 QA、容器 QA 和清理结果。
