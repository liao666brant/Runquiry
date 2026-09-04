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
    - 进程基线对齐：生产 `LinuxPlatform::new()` 使用 sysinfo 枚举 PID，再由可注入 ProcFs 逐 PID 补齐字段；`with_injected` 的合成树测试继续从注入根枚举，避免测试读取真实 `/proc`。原 `/proc` 生产枚举偏差已移除；sysinfo 用户表仍为每轮一次并复用。
    - `linux/`：`LinuxPlatform` 实现七个只读边界（ProcessInventory/ProcessDetailsProvider/NetworkInventory/FileInventory/ProcessFileLocks/SourceEvidenceProvider/ContainerProcessVerifier），`new()`（生产 `/proc` + sysinfo PID）与 `with_injected(proc_root, systemd_run_dir, own_pid)`（合成树）。子模块：`procfs/`（可注入根 + process_files/netparse/locktable 纯解析，CLK_TCK=100、PAGE_SIZE=4K 换算）、`process.rs`+`summary.rs`+`details.rs`（基线、字段诊断与详情）、`fdscan.rs`、`network.rs`、`locks.rs`、`source/`、`container.rs`、`capabilities.rs`。全部 cfg 隔离于 linux 模块。
    - 自身排除策略：生产构造时保存 sysinfo PID 基准与真实墙钟；`list()` 排除自身 PID + 「不在基准快照且（a）PPID 链可达自身，或（b）启动时刻晚于构造时刻」的进程。时间窗兜底覆盖父进程退出后被收养的辅助进程；合成测试可用 `with_constructed_at` 注入确定性时刻。
    - 健康标签（parity witr process_linux.go:193-200）：Z/T → Zombie/Stopped；healthy 时累计 CPU（utime+stime ticks ÷ CLK_TCK）> 2h → HighCpu（优先）、RSS > 1GiB → HighMem。
    - 覆盖：cmdline/comm/stat/status、cwd/exe/environ、PPID/启动时间（btime+ticks）、FD/FD limit（unlimited=0）/statm 内存/io、cgroup→ContainerContext 与容器 PID 归属校验、CapEff→capabilities、/proc/net 四表+unix、/proc/locks 三类锁、exe_deleted、Z/T 健康状态、systemd 探测 + zbus blocking D-Bus 富化。D-Bus 连接和方法调用设 2s timeout，外层只允许一个 in-flight worker；调用方 2s 返回后不会继续累积脱离线程。
    - 部分成功红线：进程身份所需 stat 失败返回硬错误；cwd/exe/environ/statm/meminfo/io/fd/limits/children 等可选字段失败则保留 `ProcessDetails` 并逐项追加 permission_denied/unknown/parse_failed，`analyze` 会合并这些诊断。进程列表单条目失败同样不抹掉其他数据；无 sudo、无 B7 操作。
    - 测试：`linux_adapters` 23 个 + Linux 专属库单测 3 个（合成 `/proc` tempdir 树，不读真实 `/proc`、无 root 依赖、无阻塞 sleep）；新增生产构造器 sysinfo/墙钟断言、字段级详情诊断、候选 PID cgroup 精确读取、systemd 超时与 single-flight 回归。超长集成测试已按 process/details/network/locks/source/container 职责拆分，单文件不超过 250 纯代码行。
    - 真实 QA（普通用户，2026-09-04）：`linux_qa` 从真实 `/proc` 读取 14 条进程且基线诊断为 0，自身 PID 已排除；真实详情、子进程、Socket、开放端口与文件锁路径均完成，环境变量仅报数量不打印值。该次环境未发现开放端口或文件锁，详情保留数据并带 1 条字段诊断，符合部分成功语义。
    - 已知限制：PAGE_SIZE 固定 4K（非 4K 内核 RSS 按比例偏差）；cpu_percent 恒 None（两样本差分属上层）；logind 阻止睡眠检测（ResourceContext 组成部分）未实现；D-Bus 富化是有界 best-effort，失败只省略键。

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
