# 模块 04：Linux 平台与 Unix 进程控制

上级方案：[Runquiry 桌面化实施计划](../runquiry-gpui-desktop.md)

## 模块目标

完成 Linux 系统采集、进程控制和 Linux 纵向验收，为 macOS、Windows 扩展前建立第一个真实可用闭环。

## 所有权与边界

允许写入：

- crates/runquiry-platform/src/linux/
- crates/runquiry-platform/src/unix/
- crates/runquiry-platform/tests/linux_*
- crates/runquiry-platform/tests/unix_*
- Linux QA 证据文件

不得写入：

- UI 组件与页面。
- 容器运行时内部实现。
- 根 Cargo 配置和 Cargo.lock。
- macOS、Windows 模块。

进程动作 QA 只能针对本任务启动并记录身份的临时进程，禁止选择系统或用户现有进程。

## TODOs

- [ ] **B2 Linux 系统适配器**
  - 依赖：A3、A5。
  - 进程列表以 sysinfo 为基线，排除 Runquiry 自身和由自身采集产生的短生命周期辅助进程。
  - 通过 procfs 补齐 cmdline、cwd、exe、环境、启动时间、父子关系、FD、Socket、资源、cgroup 和 capabilities。
  - 解析 /proc/net 与 FD inode，建立 TCP/UDP、地址、端口、状态和 PID 关系。
  - 解析 /proc/locks 和打开 FD；同一 PID/path 同时出现时锁记录优先。
  - 通过 systemd D-Bus、cron、supervisor 和祖先链提供启动来源证据。
  - 权限不足、进程消失和单个 /proc 文件损坏均返回部分结果。
  - 验证：
    - cargo test -p runquiry-platform linux --locked
    - 普通用户运行平台 integration tests
    - 对受限进程验证 PermissionDenied/Partial，而不是空集合
  - 完成证据：能力矩阵、fixture 测试、普通用户真实采集结果。

- [ ] **B7 Unix 进程控制**
  - 依赖：B2、B5。
  - 实现 SIGTERM、SIGKILL、SIGSTOP、SIGCONT 和 setpriority。
  - 执行前重新读取 ProcessIdentity；PID、启动时间或可执行文件不一致时返回 ProcessChanged。
  - renice 仅接受 -20..=19。
  - 权限不足不重试、不调用 sudo、不弹出系统提权。
  - 成功后通知 UI 立即刷新目标和当前工作区。
  - 验证：
    - cargo test -p runquiry-platform process_controller --locked
    - 对任务自建临时进程依次验证 pause、resume、terminate
    - 对第二个自建进程验证 kill
    - 使用假身份验证 PID 复用拒绝路径
  - 完成证据：临时 PID 清单、动作前后状态、全部清理回执。

- [ ] **B8 Linux 纵向验收门**
  - 依赖：B1-B7。
  - 在 Ubuntu 22.04+ 分别使用 X11 与 Wayland 启动完整应用。
  - 验证四个工作区、五类调查、排序、过滤、详情、主题、中英切换和自适应刷新。
  - 分别验证有 Docker 与无任何容器 CLI 的主机状态。
  - 验证普通用户权限不足路径。
  - 使用本任务临时进程完成操作确认流程。
  - 用大 fixture 验证 100k 行虚拟滚动与持续刷新。
  - 验证结束后关闭应用、临时进程、容器和显示会话。
  - 完成证据：X11/Wayland 截图、场景结果、性能观察、清理回执。

## 模块退出条件

- B8 全部通过前不得开始 C1、C2。
- Linux 上的四页面和五类调查均有真实运行证据。
- 没有自动提权、遗留子进程或权限错误静默降级。

## 交接格式

- Linux 能力与已知限制。
- 实机发行版、桌面会话和显示协议。
- 自动测试、临时进程 QA、X11/Wayland QA 结果。
- 未通过场景；正常完成时应为“无”。
