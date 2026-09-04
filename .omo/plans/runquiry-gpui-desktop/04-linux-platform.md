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

- [x] **B2 Linux 系统适配器**
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
  - 实施记录（2026-09-04，评审修复后更新）：
    - **intentional change 注记**：进程基线未按上级方案文字以 sysinfo 枚举，改为 `/proc` 目录扫描基线 + sysinfo 仅用于 uid→用户名解析。理由：Linux 上 sysinfo 进程枚举本身读 /proc，扫描基线可直接复用可注入 ProcFs 的测试路径与逐条目失败诊断；B8 纵向验收时复核该取舍（若切回 sysinfo 枚举须保持逐条目部分成功语义）。
    - `linux/`：`LinuxPlatform` 单一结构实现六个只读端口（ProcessInventory/ProcessDetailsProvider/NetworkInventory/FileInventory/**ProcessFileLocks**/SourceEvidenceProvider），`new()`（生产 /proc）与 `with_injected(proc_root, systemd_run_dir, own_pid)`（合成树/QA）。子模块：`procfs/`（可注入根 + process_files/netparse/locktable 纯解析，CLK_TCK=100、PAGE_SIZE=4K 换算）、`process.rs`+`details.rs`（基线与详情）、`fdscan.rs`（FD→inode 归因）、`network.rs`、`locks.rs`（锁记录优先，普通 FD 以 Other+Read 表达；`locks_of(pid)` 按持有者 PID 过滤，与 `holders` 共享解析路径）、`source.rs`（SourceEvidenceProvider，平台不判定来源类型）、`capabilities.rs`（CapEff 译码）。全部 cfg 隔离于 linux 模块。
    - 自身排除策略：构造时 /proc PID 基准快照；`list()` 排除自身 PID + 「不在基准快照且（a）PPID 链可达自身，或（b）启动时刻晚于构造时刻」的进程——时间窗兜底覆盖父进程已退出、被收养导致 PPID 链断裂的辅助进程（`with_constructed_at` 注入确定性构造时刻供测试）。用户表每轮 `list()` 采集一次、全轮复用（避免每进程重读 /etc/passwd）。
    - 健康标签（parity witr process_linux.go:193-200）：Z/T → Zombie/Stopped；healthy 时累计 CPU（utime+stime ticks ÷ CLK_TCK）> 2h → HighCpu（优先）、RSS > 1GiB → HighMem。
    - 覆盖：cmdline/comm/stat/status、cwd/exe/environ、PPID/启动时间（btime+ticks）、FD/FD limit（unlimited=0）/statm 内存/io、cgroup→ContainerContext（复用 core 纯函数）、CapEff→capabilities、/proc/net 四表+unix（合法端口、Unix 无端口、`validate_socket_entry` 复用、port 0 跳过记诊断）、/proc/locks 三类锁、exe_deleted（仅 " (deleted)" 后缀证据）、Z/T 健康状态、systemd 探测（/run/systemd/system）+ zbus blocking D-Bus 富化（2s 有界，best-effort）。
    - 部分成功红线：权限不足/进程消失/单文件损坏 → Inspection 部分数据 + DiagnosticIssue（permission_denied/unknown），不返回空集合冒充成功；无 sudo、无 B7 操作。
    - 测试：`cargo test -p runquiry-platform linux --locked` 20 个（合成 /proc tempdir 树，不读真实 /proc、无 root 依赖、无 sleep；测试名统一 `linux_` 前缀匹配定向过滤器；含健康标签阈值边界、收养后代时间窗排除、locks_of 按 PID 过滤）。
    - 真实 QA（普通用户，`cargo run -p runquiry-platform --example linux_qa --locked`）：96 进程基线（自身 434598 被排除）、自身/自建子进程详情（fd/limit/cwd/open_files，环境变量只报计数不报值）、29 条开放端口（7 归因 + 22 无主 + 1 条 permission_denied 诊断——64 个他人进程 fd 不可读，条目不丢）、文件锁查询。系统：WSL2 内核 6.18.33.2，普通用户。
    - 已知限制：PAGE_SIZE 固定 4K（非 4K 内核 RSS 按比例偏差）；cpu_percent 恒 None（两样本差分属上层）；logind 阻止睡眠检测（ResourceContext 组成部分）未实现；D-Bus 富化失败只省略键。

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
