# Runquiry：witr 的 Rust + GPUI 桌面化实施计划

## 摘要

Runquiry 是对本地 [witr 源码](../../witr/README.md) 的纯 Rust 桌面化重写。产品只提供原生桌面 GUI，不嵌入 Go，也不新增 Rust CLI/TUI。实现顺序为 Linux 纵向闭环优先，随后并行补齐 macOS 和 Windows，最终交付三个平台的无签名安装包。

本计划的目标是让多个 Agent 可以按明确所有权和依赖关系并行开发。每个任务都必须独占其声明的文件或模块；共享 Cargo 配置、依赖锁和最终集成只由集成负责人修改。

## 固定技术与产品决策

- 技术栈：Rust edition 2024、稳定工具链、Zed GPUI、gpui-component。
- 不混用 Glass-HQ GPUI。gpui-component 官方文档使用 Zed GPUI，并要求先初始化 gpui-component，再以 Root 作为每个窗口的第一级视图：[安装说明](https://longbridge.github.io/gpui-component/docs/installation)、[Root 契约](https://longbridge.github.io/gpui-component/docs/root)。
- 初始依赖基线：
  - gpui-component 提交：91217366a5765600a127bf108ce00b7143a93381。
  - Zed GPUI 锁文件提交：f66ed399cdde86092af8af3dc7b418abf45f37f8。
  - Cargo.lock 必须提交，CI、测试和打包全部使用 --locked；功能任务不得顺带升级依赖。
- 最终平台：Linux、macOS、Windows。FreeBSD 不在本期范围。
- 产品界面：Processes、Ports、Containers、File Locks 四个工作区，以及按名称、PID、端口、文件、容器发起的调查入口。
- 保留 witr 的祖先链、启动来源、容器上下文、资源、Socket、文件、环境变量和风险告警。
- Linux/macOS 支持 terminate、kill、pause、resume、renice；Windows 明确标记为不支持。
- 运行期完全本地：不包含遥测、云同步、远程诊断、自动更新或后台网络请求。
- 交付 Linux AppImage/DEB、macOS APP/DMG、Windows MSI；全部无签名，不上传应用商店或创建远程 Release。

### 许可证决策

当前选定的 GPUI 依赖链包含 GPL-3.0-or-later 的 zlog、ztracing、ztracing_macro。处理方式如下：

- Runquiry 分发许可证采用 GPL-3.0-or-later。
- 在 [deny.toml](../../deny.toml) 中仅为上述三个包、固定版本和固定 Git 来源添加例外，不全局放开 GPL。
- 保留 witr 的 Apache-2.0 许可证、版权归属及 NOTICE。
- 使用依赖清单生成第三方许可证文件。
- 公开发布前对最终 Cargo.lock 进行一次人工许可证复核。

相关上游问题记录：[gpui-component issue 2475](https://github.com/longbridge/gpui-component/issues/2475)。

## 架构

### Workspace 分层

~~~text
runquiry-app
├── runquiry-ui ───────────────┐
├── runquiry-platform ───────┐ │
└── runquiry-core <──────────┴─┘

runquiry-core
    领域模型、目标解析、分析管线、告警规则、刷新状态机、平台端口

runquiry-platform
    Linux/macOS/Windows 采集器、容器运行时、进程控制、外部命令执行

runquiry-ui
    GPUI 状态、设计系统、四个工作区、调查面板、设置与国际化

runquiry-app
    依赖装配、窗口启动、配置持久化、资源和打包元数据
~~~

依赖方向保持单向：

- runquiry-core 不依赖 GPUI 或操作系统实现。
- runquiry-platform 和 runquiry-ui 只依赖 runquiry-core。
- runquiry-app 负责装配平台实现、应用服务和 UI。
- UI 不允许直接读取 /proc、调用 Win32 API、运行 lsof 或容器 CLI。

### 公共领域类型

- Pid(u32)
- Port(u16)
- ContainerKey { runtime, id }
- QueryTarget：
  - ProcessName { query, exact }
  - Pid
  - Port
  - File(PathBuf)
  - Container { query, exact }
- ProcessIdentity { pid, start_time, executable }：破坏性操作执行前重新读取，防止 PID 复用误操作。
- Inspection<T> { data, issues, captured_at }：支持部分成功，单个权限或工具错误不得丢弃其余数据。
- CapabilityStatus：
  - Supported
  - Partial(reason)
  - Unsupported(reason)
  - Unavailable(reason)
- InspectError：
  - InvalidTarget
  - NotFound
  - Ambiguous
  - PermissionDenied
  - Unsupported
  - ExternalTool
  - ProcessChanged
- ProcessAction：
  - Terminate
  - Kill
  - Pause
  - Resume
  - Renice(i8)，仅允许 -20..=19

### 平台接口

按职责拆分以下同步接口，避免单一胖接口：

- ProcessInventory
- ProcessDetailsProvider
- NetworkInventory
- ContainerInventory
- FileInventory
- ProcessController
- CommandRunner

同步接口统一由 GPUI 后台执行器调度。UI 只接收不可变快照、部分失败信息和 generation，不直接持有平台资源。

### 平台能力矩阵

| 能力 | Linux | macOS | Windows |
|---|---|---|---|
| 进程基线 | sysinfo | sysinfo | sysinfo |
| 深度信息 | procfs、cgroup、capabilities | libproc、plist | windows crate、PEB |
| 端口与 Socket | /proc/net + FD inode | lsof -F | IP Helper API |
| 文件锁/打开文件 | /proc/locks、FD | lsof -F，best effort | Unsupported |
| 服务来源 | systemd D-Bus、cron、supervisor | launchd/plist | Windows SCM |
| 容器 | 实际存在的受支持 CLI | 实际存在的受支持 CLI | 实际存在的受支持 CLI |
| 进程操作 | 完整 | 完整 | Unsupported |

容器运行时范围：

- Docker
- Podman
- nerdctl
- crictl
- Incus
- LXC
- LXD

FreeBSD jail 不实现。

### 外部命令边界

所有容器 CLI、lsof 和 launchctl 调用统一经过 CommandRunner：

- 不经过 shell，只接受程序名与独立 argv。
- 可用性探测超时 500ms。
- 列表调用超时 3s。
- 详情调用超时 5s。
- 单次 stdout 和 stderr 分别限制为 8MiB。
- 某个运行时失败只生成 DiagnosticIssue，不阻断其他运行时。
- 容器按 runtime + id 去重。
- 只解析 JSON 或稳定机器格式，不解析本地化的人类输出。

### 来源识别与告警

来源与告警从 [witr 分析管线](../../witr/internal/pipeline/analyze.go) 移植为纯函数，并通过 fixture 固定行为。

来源至少覆盖：

- Container
- SSH
- Shell
- systemd
- launchd
- supervisor
- cron
- Windows Service
- init

告警至少覆盖：

- root 用户运行
- 危险 Linux capabilities
- 公开地址监听
- 高 CPU 或高内存
- 运行时间超过 90 天
- 可疑工作目录
- 容器缺少健康检查
- 进程名称与可执行文件不匹配
- 已删除的可执行文件
- LD_PRELOAD 或 DYLD_* 注入

## UI 与交互方案

### 设计方向

面向开发者和运维人员的高密度本地诊断控制台：

- DESIGN_VARIANCE = 3
- MOTION_INTENSITY = 2
- VISUAL_DENSITY = 9
- 中性石墨/灰色表面，钴蓝作为交互强调色。
- 绿、黄、红只表达成功、警告和危险等语义状态。
- 使用系统 UI 字体与系统等宽字体，保证中英文覆盖且不增加字体下载。
- 默认跟随系统浅深色主题。
- 中文系统默认 zh-CN，其他系统默认 en，英文为兜底。

语言切换通过 gpui-component 的 rust-i18n 扩展机制实现，并在设置变化后显式触发重绘：[I18n 契约](https://longbridge.github.io/gpui-component/docs/i18n)。

### 应用壳层

- 默认窗口：1280×800。
- 最小窗口：960×640。
- 固定区域：侧栏、工具栏、状态栏。
- 数据表拥有主滚动区域，详情面板拥有独立纵向滚动。
- 宽度不低于 1100 时使用 65/35 可调整列表-详情布局。
- 宽度为 960-1099 时详情通过 Sheet 展示。
- 表格采用 34px 紧凑行高、虚拟滚动、稳定 ID 选择、排序、固定列和可调整列宽。

gpui-component 已提供相应 DataTable、Tree、Sidebar、Sheet、Dialog、Notification 和 StatusBar 能力：[组件能力](https://longbridge.github.io/gpui-component/docs/)。

### 页面

1. Processes
   - 列：名称、PID、用户、CPU、内存、启动时间、来源、告警。
   - 详情：概览、祖先树、资源、Socket、文件、环境变量和操作。
   - 第一次 CPU 采样显示“采样中”，第二个有效样本后显示数值。

2. Ports
   - 列：协议、本地地址、端口、状态、PID、进程、公开监听标记。
   - 支持“仅监听”和“全部 Socket”切换。
   - 选中行后复用进程详情能力。

3. Containers
   - 列：运行时、名称、ID、状态、健康、镜像、主机 PID、启动时间。
   - 可获取主机 PID 时复用完整进程分析。
   - 无主机 PID 时展示容器自身的部分结果。

4. File Locks
   - 列：路径、类型、模式、PID、进程、FD。
   - 支持“仅锁”和“全部打开文件”切换。
   - Windows 保留页面入口并展示明确的 Unsupported 说明。

### 调查入口

顶部调查栏使用显式目标类型，不自动猜测：

- Name 和 Container 提供 exact 开关。
- PID 必须是正整数。
- Port 必须在 1-65535。
- File 支持手工路径与文件选择器。
- 多结果进入候选表，不自动选择第一项。
- 无结果、无权限、平台不支持和外部工具失败必须使用不同状态。

快捷键：

- Ctrl/Cmd+K：聚焦调查栏。
- Ctrl/Cmd+R：刷新当前工作区。
- Ctrl/Cmd+1..4：切换四个工作区。
- 方向键：移动表格选择。
- Enter：打开详情。
- Escape：关闭 Sheet 或 Dialog。

### 刷新与并发

刷新策略直接移植 [witr 的刷新常量](../../witr/internal/tui/constants.go)：

- 初始刷新间隔 3 秒。
- 最小 3 秒，最大 30 秒，每次增减 3 秒。
- 连续两次耗时超过当前间隔 60% 时退避。
- 连续两次耗时低于当前间隔 30% 时加速。
- 30%-60% 区间保持当前间隔。
- 同一工作区禁止重入刷新。
- 选择变化后延迟 500ms 获取详情。
- 每个列表和详情请求携带 generation。
- 页面、目标或选择变化后，旧 generation 的结果必须被丢弃。
- 手工刷新不得与正在执行的自动刷新并行。

### 安全交互

环境变量和命令行中的以下键默认脱敏：

- token
- secret
- password
- passwd
- pwd
- auth
- cookie
- key
- credential
- session

匹配不区分大小写。揭示只对当前详情会话生效，不写入设置、不写入日志。

进程操作采用两步流程：

1. 用户从操作菜单选择动作。
2. AlertDialog 展示进程名、PID、用户、启动时间、可执行路径和动作，再要求确认。

执行确认前重新获取 ProcessIdentity。PID 不存在或身份变化时立即中止。应用不自动提权；权限不足只返回可操作的错误说明。

## 模块任务文件与批次编排

### 执行约定

- 总计划是架构、产品边界、平台矩阵和全局验收标准的唯一规范来源。
- 模块文件是任务内容、任务状态、模块验收和 Agent 交接的唯一规范来源。
- 执行某项任务前，Agent 必须阅读本总计划以及该任务所在的模块文件。
- 一个任务同一时间只允许一个 Agent 写入；并行任务不得修改对方的所有权范围。
- 根 Cargo 配置、Cargo.lock、工具链和许可证只由 A1 负责人修改；最终集成阶段只由 D4 负责人处理共享冲突。
- 完成任务时只勾选所属模块文件中的任务，不在总计划复制进度。
- 每项交接必须包含变更文件、自动验证、真实 QA、已知风险和未完成项。

### 模块索引

| 模块文件 | 包含任务 | 主要所有权 |
|---|---|---|
| [01-foundation.md](runquiry-gpui-desktop/01-foundation.md) | A1、A2、A5 | Workspace、依赖许可、行为契约、fixture |
| [02-core-analysis.md](runquiry-gpui-desktop/02-core-analysis.md) | A3、B1 | runquiry-core、目标解析、分析管线 |
| [03-container-runtime.md](runquiry-gpui-desktop/03-container-runtime.md) | B3 | CommandRunner、容器运行时 |
| [04-linux-platform.md](runquiry-gpui-desktop/04-linux-platform.md) | B2、B7、B8 | Linux 采集、Unix 控制、Linux 验收 |
| [05-desktop-ui.md](runquiry-gpui-desktop/05-desktop-ui.md) | A4、B4、B5、B6、C3 | DESIGN.md、GPUI 壳层、四工作区、能力 UI |
| [06-macos-platform.md](runquiry-gpui-desktop/06-macos-platform.md) | C1 | macOS 采集和控制 |
| [07-windows-platform.md](runquiry-gpui-desktop/07-windows-platform.md) | C2 | Windows 采集和能力限制 |
| [08-integration-release.md](runquiry-gpui-desktop/08-integration-release.md) | C4、D1、D2、D3、D4 | 跨平台回归、硬化、打包、最终验收 |

### 批次顺序

| 批次 | 可执行任务 | 完成条件 |
|---|---|---|
| Batch 0 | A1 与 A2 并行 | Workspace/依赖基线和 witr 行为契约均完成 |
| Batch 1 | A3 与 A4 并行 | 核心接口冻结，设计系统实验台通过视觉 QA |
| Batch 2 | A5 与 B4 并行 | Fixture/test harness 和应用状态骨架可用 |
| Batch 3 | B1、B2、B3 并行 | 分析管线、Linux 采集、容器运行时分别通过模块测试 |
| Batch 4A | B5 与 B6 并行 | 四个工作区完成 Linux 数据闭环 |
| Batch 4B | B7 | 进程操作安全验证通过 |
| Batch 5 | B8 | Linux X11、Wayland 真实验收通过 |
| Batch 6 | C1 与 C2 并行 | macOS、Windows 模块分别通过实机验收 |
| Batch 7A | C3 | 平台能力正确映射到 UI |
| Batch 7B | C4 | 三平台契约回归通过并冻结 v1 行为 |
| Batch 8 | D1 与 D2 并行 | 安全/性能硬化和三平台打包均完成 |
| Batch 9A | D3 | 用户、维护和许可证文档完成 |
| Batch 9B | D4 | 全量自动检查、安装包和实机 QA 全部通过 |

### 依赖主链

~~~text
A1 ─┬─> A3 ─┬─> A5 ─┬─> B1 ───────────┐
    │       │       ├─> B2 ─┬─> B5 ─┐ │
    └─> A4 ─┴─> B4 ─┘       ├─> B6 ─┼─> B7 ─> B8
A2 ────────> A3              │       │
A5 ───────────────> B3 ──────┘       │
                                      ├─> C1 ─┐
                                      └─> C2 ─┴─> C3 ─> C4
                                                       ├─> D1 ─┐
                                                       └─> D2 ─┴─> D3 ─> D4
~~~

并行只依据该依赖表判断。即使任务位于同一模块文件，也不得越过未完成的前置任务。
## 必测场景

- 五类目标的合法、非法、无结果和多结果。
- 进程在列表刷新、详情加载或操作确认期间退出。
- PID 被复用时拒绝执行进程操作。
- 首次 CPU 样本、后续样本和刷新退避/恢复。
- 单个容器运行时失败但其他运行时正常。
- 无权限、缺少外部 CLI、超时和部分数据。
- Windows File Locks 和进程操作的明确 Unsupported 表现。
- 中英切换后当前视图立即重绘且没有缺失 key。
- 环境变量与敏感命令参数默认脱敏，重启后不泄露调查数据。
- 960×640、1280×800、超宽窗口、浅色和深色主题。
- 空列表、100k 行、40 字符进程名、无空格长路径和中文路径。
- Linux X11、Linux Wayland、macOS 和 Windows 的安装、启动、卸载及离线运行。
- 安装包校验和与第三方许可证文件。

## 完成标准

- 四个工作区和五类调查在对应支持平台真实运行。
- 主线程不执行系统扫描，滚动和输入期间无可感知冻结。
- 旧 generation 不能覆盖新页面、新目标或新选择。
- 平台限制、权限错误、部分结果和真正的空集合可以区分。
- 无 unwrap、expect、panic、未说明的 unsafe 或占位 TODO。
- 核心与平台模块控制在约 250 行纯代码以内，超过时按单一职责拆分。
- 三个平台安装包均经过真实启动和关键路径 QA，而不只通过编译。

## 假设与边界

- 应用名为 Runquiry，二进制名为 runquiry，bundle identifier 为 dev.runquiry.app。
- 本地 witr/ 只作为行为和许可来源，继续保持未跟踪状态。
- Rust 使用 edition 2024 和稳定工具链，不使用 nightly。
- A1 从 Rust 1.90.0 开始选择第一个能编译固定依赖组合的稳定版本，并锁定精确 patch 版本。
- 不存在或不兼容的 OS 能力通过 CapabilityStatus 表达，不通过伪数据、静默空集合或命令模拟。
- 无签名安装器会触发 macOS Gatekeeper 和 Windows SmartScreen 提示。
- 签名、公证、应用商店、自动更新和密钥管理属于后续独立项目。
- 本计划不包含 Git 分支、commit、push、远程 Release 创建或生产发布操作。
