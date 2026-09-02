# 模块 07：Windows 平台

上级方案：[Runquiry 桌面化实施计划](../runquiry-gpui-desktop.md)

## 模块目标

补齐 Windows 10 22H2+ x64 的进程、环境、端口、服务来源和容器上下文，并把无法安全提供的文件锁与进程操作明确建模为 Unsupported。

## 所有权与边界

允许写入：

- crates/runquiry-platform/src/windows/
- crates/runquiry-platform/tests/windows_*
- Windows fixture 与 QA 证据

不得写入：

- core 领域规则、UI、Linux/macOS 适配器。
- 容器运行时通用实现。
- 根 Cargo 配置和 Cargo.lock。
- 伪造 File Locks 或通过命令行模拟 Unix signal。

Windows 专用依赖和 feature 由 A1 负责人集中维护。

## TODOs

- [ ] **C2 Windows 适配器**
  - 依赖：B8。
  - 使用 sysinfo 提供基线进程列表。
  - 通过 windows crate 与 PEB 安全包装读取 exe、cmdline、cwd、环境、父进程和启动时间。
  - 使用 IP Helper API 获取 TCP/UDP、本地地址、端口、状态和 PID。
  - 使用 Windows SCM 获取服务归属和启动来源证据。
  - 复用模块 03 的 Docker、Podman、nerdctl 等容器适配器。
  - FileInventory 的锁枚举和 ProcessController 返回 CapabilityStatus::Unsupported。
  - 访问拒绝、32/64 位边界、进程退出和受保护进程返回部分结果。
  - FFI 只存在于 windows 子模块的安全包装中，每个 unsafe 块说明指针、缓冲区和生命周期条件。
  - 验证：
    - cargo test -p runquiry-platform windows --locked
    - Windows 10 22H2+ x64 实机读取进程、端口、服务和容器
    - 普通用户读取受保护进程，确认保留部分数据
    - 验证 File Locks 和进程操作始终为 Unsupported
  - 完成证据：实机记录、受保护进程状态、FFI 审核清单、无遗留进程。

## 必测场景

- 32 位目标进程与 64 位 Runquiry。
- PEB 在读取过程中变化或进程退出。
- 环境块无终止符、包含非 BMP 字符或访问拒绝。
- IPv4、IPv6、TCP、UDP 和监听/连接状态。
- SCM 服务不存在、停止或权限不足。
- Docker Desktop 未安装、未启动、已启动。
- Windows File Locks 页面所需的 Unsupported 原因键稳定。

## 模块退出条件

- Windows 实机完成进程、端口、来源、容器和调查入口验证。
- File Locks 与进程操作无空实现、TODO 或误导性按钮。
- FFI 与敏感内存读取经过独立审查。

## 交接格式

- Windows 能力矩阵与 Unsupported 项。
- 实机版本、架构和权限级别。
- 自动测试、真实 QA、FFI 审核和已知限制。
