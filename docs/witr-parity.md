# witr 行为契约（witr-parity）

## 目的

本文件是 Runquiry（witr 的 Rust + GPUI 桌面化重写）的行为契约来源。它逐项记录本地参考实现 [witr 源码](../witr/README.md) 中可观察的行为与能力，标注每项在 Runquiry 中的处置方式，使其他 Agent 可以只读取上级方案、本契约和 fixture 开始工作，无需重新决定依赖、许可证或 witr 语义。

规范来源为：

- 上级方案：[Runquiry 桌面化实施计划](../.omo/plans/runquiry-gpui-desktop.md)（架构、产品边界、平台矩阵与全局验收标准）。
- 模块任务文件：[01-foundation.md](../.omo/plans/runquiry-gpui-desktop/01-foundation.md)（A2 要求：四工作区、五类目标、来源优先级、告警、容器运行时、刷新策略、进程操作和平台差异）。

## 状态图例

每个条目只有以下三种状态之一：

| 状态 | 含义 |
|---|---|
| `parity` | 与 witr 行为一致，Runquiry 需按原语义实现 |
| `intentional change` | 有意偏离 witr 的行为（例如 CLI 提示改为 GPUI 候选表、退出码改为类型化错误） |
| `out of scope` | 本期不实现，或属于被明确排除的平台/入口形态（含 macOS 专属行为） |

## 生成说明

- 本矩阵基于对 `witr/` 源码（`pkg/model`、`internal/target`、`internal/pipeline`、`internal/proc`、`internal/source`、`internal/tui`、`internal/app`、`internal/output` 及对应测试）的**静态阅读**整理，不包含任何运行态数据，不复制整段 Go 实现。
- 每个条目的证据列均链接到 `witr/` 下的具体源码文件与符号；无 witr 对应行为的条目标注为 `out of scope` 并在证据列说明。
- `out of scope` 条目保留以说明边界：FreeBSD 不在本期范围，GUI 应用无 `--version` / 补全 / doc 生成入口，`json:"..."` 输出契约不保留，`Exit*` 数字退出码不进入 GUI。
- 跨章节重复的条目保留在主要主题章节，重复行的证据列以「同 §X 对应条」标注来源；同一章节内的逐字重复行已删除。
- **macOS 已移出 v1 范围（2026-09-08）**：纯 macOS 专属条目统一改标为 `out of scope`；跨平台条目中提及 macOS 实现方式的备注保留原文（记录 witr 行为事实），但其 macOS 部分不在 Runquiry 实现范围内。平台矩阵为 Linux 与 Windows。

## 1. 领域模型

| 行为或能力 | 状态 | witr 证据 | Runquiry 对应模块 | 平台差异或说明 |
|---|---|---|---|---|
| Pid(u32) 与 Port(u16) 提升为新类型，替代 witr 各模型里的裸 int/int 字段 | intentional change | [`Process.PID`](../witr/pkg/model/process.go) 与 [`Socket.Port`](../witr/pkg/model/socket.go)（Go 侧均为裸 int 字段，`Pid(u32)`/`Port(u16)` 为 Runquiry 新类型） | core 领域模型（总计划·公共领域类型） | 无 |
| 进程身份基线字段：PID、PPID、命令名、完整命令行、可执行路径、启动时间、用户 | parity | [`Process{PID, PPID, Command, Cmdline, Exe, StartedAt, User}`](../witr/pkg/model/process.go) | core 领域模型 + platform 进程基线 | 无（sysinfo 三平台统一提供基线） |
| 资源基线字段：CPU 百分比、RSS 字节数、内存百分比 | parity | [`Process.CPUPercent / MemoryRSS（注释 In bytes）/ MemoryPercent`](../witr/pkg/model/process.go) | core 领域模型 + ui Processes 工作区 | RSS 单位统一为字节 |
| 工作上下文字段：工作目录、Git 仓库、Git 分支、容器名、服务名 | parity | [`Process{WorkingDir, GitRepo, GitBranch, Container, Service}`](../witr/pkg/model/process.go) | core 领域模型（总计划·保留清单） | 无 |
| Git 仓库与分支上下文（GitRepo/GitBranch）：总计划的保留清单与四个工作区详情枚举均未提及 Git 上下文 | out of scope | [`Process.GitRepo / Process.GitBranch`](../witr/pkg/model/process.go) | core 领域模型 / ui Processes 详情 | 无 |
| 容器身份与运行时健康检查判定字段：`ContainerID`、`ContainerRuntime` 与 `ContainerHealthcheck`（注释含 cgroup 与 `""`, `"present"`, `"absent"`） | parity | [`Process.ContainerID / ContainerRuntime / ContainerHealthcheck`](../witr/pkg/model/process.go) | core 容器上下文 + platform Linux 采集 | witr 明示取自 Linux cgroup 检测 |
| 进程持有的全部 Socket（LISTEN、ESTABLISHED、CLOSE_WAIT 等），每条含协议与状态 | parity | [`Process.Sockets` 与注释（every socket the process owns）](../witr/pkg/model/process.go) | core 领域模型 + ui Processes 详情 / Ports 工作区 | 采集路径平台各异 |
| 健康状态字符串枚举（healthy、zombie、stopped、high-cpu、high-mem） | parity | [`Process.Health`（注释列出五种取值）](../witr/pkg/model/process.go) | core 告警与状态 | zombie/stopped 依赖各平台进程状态采集 |
| fork 状态枚举（forked / not-forked / unknown，仅在 forked 时于 witr 输出中展示） | out of scope | [`Process.Forked`（注释列出三值）](../witr/pkg/model/process.go) | core 领域模型（总计划未列） | Windows 恒 `unknown` |
| 环境变量列表（Env，key=value） | parity | [`Process.Env`（注释 Environment variables (key=value)）](../witr/pkg/model/process.go) | core 领域模型 + ui 详情与脱敏 | 无 |
| 标记进程启动后可执行文件已被删除 | parity | [`Process.ExeDeleted`](../witr/pkg/model/process.go) | core 告警规则 | Linux 经 /proc/PID/exe；macOS/Windows 需各自探测途径，告警行为三平台保留 |
| Linux capabilities 列表（如 CAP_NET_BIND_SERVICE、CAP_SYS_ADMIN） | parity | [`Process.Capabilities`](../witr/pkg/model/process.go) | core 告警 + platform Linux 采集 | capabilities 为 Linux 特有概念，其他平台 Capabilities 为空 |
| 详细内存信息：VMS/RSS/Shared/Text/Lib/Data/Dirty 字节数及 VMSMB/RSSMB 换算值 | parity | [`MemoryInfo`](../witr/pkg/model/process.go) | core 领域模型 + ui Processes 资源详情 | 无 |
| I/O 统计：读写字节数与读写操作次数 | parity | [`IOStats{ReadBytes, WriteBytes, ReadOps, WriteOps}`](../witr/pkg/model/process.go) | core 领域模型 + ui 资源详情 | Linux /proc/PID/io |
| 打开文件描述符列表、FD 数量与 FD 上限（verbose 附加信息） | parity | [`Process.FileDescs / FDCount / FDLimit`](../witr/pkg/model/process.go) | core 领域模型 + ui 文件详情 | Linux /proc/locks、FD；macOS lsof best effort；Windows Unsupported |
| 子进程 PID 列表（verbose 附加信息） | parity | [`Process.Children []int`](../witr/pkg/model/process.go) | core 领域模型 + ui 祖先树 | 无 |
| 线程数量统计（verbose 附加信息） | out of scope | [`Process.ThreadCount`](../witr/pkg/model/process.go) | core 领域模型（总计划未列） | 无 |
| 模型字段带 `json:"...,omitempty"`（ContainerID/ContainerRuntime/ContainerHealthcheck/Memory/IO/FileDescs/FDCount/FDLimit/Children/ThreadCount） | out of scope | [`ContainerID/ContainerRuntime/ContainerHealthcheck/Memory/IO/FileDescs/FDCount/FDLimit/Children/ThreadCount 的 json:"...,omitempty"`](../witr/pkg/model/process.go)、[`ComposeProject 等字段的 json:"...,omitempty"`](../witr/pkg/model/container.go)、[`Result.Children` 的 `json:"...,omitempty"`](../witr/pkg/model/result.go) | core 领域模型（总计划·固定技术与产品决策：不新增 CLI，无 JSON 导出契约） | 无 |
| 进程内 Socket 条目字段（Inode、Port、Address、State、Protocol） | parity | [`Socket{Inode, Port, Address, State, Protocol}`](../witr/pkg/model/socket.go) | core 领域模型 + ui Ports 工作区 | Inode 仅 Linux 有意义 |
| 端口查询的 Socket 状态快照（Port、State、LocalAddr、RemoteAddr） | parity | [`SocketInfo{Port, State, LocalAddr, RemoteAddr}`](../witr/pkg/model/socket.go) | core 领域模型 + ui Ports 工作区 | 无 |
| SocketInfo 附带人类可读解释与排查建议（Explanation/Workaround） | out of scope | [`SocketInfo.Explanation`（注释 Human-readable explanation of the state）与 `SocketInfo.Workaround`（注释 Suggested workaround if applicable）](../witr/pkg/model/socket.go) | ui 文案（CLI 报告文案，桌面不保留） | 状态名为 Linux TCP 状态集 |
| 开放端口条目（PID、Port、Address、Protocol、State） | parity | [`OpenPort{PID, Port, Address, Protocol, State}`](../witr/pkg/model/net.go) | core 领域模型 + ui Ports 工作区 | Linux /proc/net + FD inode；macOS lsof -F；Windows IP Helper API |
| 容器匹配结果（ContainerMatch） | parity | [`ContainerMatch`](../witr/pkg/model/container.go) | core 领域模型 + ui Containers 工作区 | 无 |
| Compose 项目信息（ComposeProject / ComposeService / ComposeConfigFile / ComposeWorkingDir） | out of scope | [`ComposeProject / ComposeService / ComposeConfigFile / ComposeWorkingDir`](../witr/pkg/model/container.go) | core 容器上下文（总计划未列） | 无 |
| 启动来源结构（Type/Name/Description/UnitFile/Details） | parity | [`Source{Type, Name, Description, UnitFile, Details map[string]string}`](../witr/pkg/model/source.go) | core 来源识别 | UnitFile 为 systemd 语义；macOS 用 launchd/plist，Windows 用 SCM |
| unknown 兜底来源 | parity | [`SourceUnknown SourceType = "unknown"`](../witr/pkg/model/source.go) | core 来源识别（平台限制、权限错误、部分结果和真正的空集合可以区分） | 无 |
| macOS 资源上下文（EnergyImpact/PreventsSleep/ThermalState/AppNapped/CPUUsage/MemoryUsage） | out of scope | [`ResourceContext`（注释 holds resource usage context for a process）](../witr/pkg/model/resource.go) | 无（macOS 支持已移出 v1 范围） | macOS 专属（EnergyImpact/ThermalState/AppNap 为 macOS 概念） |
| 文件锁行模型（PID、进程名、路径、锁类型 POSIX/FLOCK/OFDLCK、模式 READ/WRITE/RW） | parity | [`LockedFile{PID, Process, Path, Type, Mode}`](../witr/pkg/model/lock.go) | core 领域模型 + ui File Locks 工作区 | Linux /proc/locks；macOS lsof -F best effort；Windows Unsupported（保留页面入口并显示说明） |
| 进程级文件上下文（打开文件数、FD 软上限、锁文件路径列表） | parity | [`FileContext{OpenFiles, FileLimit, LockedFiles}`](../witr/pkg/model/filecontext.go) | core 领域模型 + ui Processes 文件详情 | Linux /proc/PID/fd；macOS lsof best effort；Windows Unsupported |
| FD 上限获取（进程自身 limit 优先，失败回退系统默认） | parity | [`getFileLimit / getDefaultMaxOpenFiles`](../witr/internal/proc/filecontext_linux.go)、[`filecontext_darwin.go 的 getFileLimit（默认 256）`](../witr/internal/proc/filecontext_darwin.go)、[`filecontext_freebsd.go 的 getFileLimit（默认 1024）`](../witr/internal/proc/filecontext_freebsd.go)、[`extended_linux.go 的 ReadExtendedInfo（fdLimit 复用 getFileLimit）`](../witr/internal/proc/extended_linux.go) | platform（页面 Processes 详情「文件」） | 各平台来源不同，Linux 读 /proc/<pid>/limits 的 "Max open files" 软限制（unlimited 记为 0） |
| 扩展进程信息：内存明细、磁盘 IO 计数、FD 列表与数量、FD 上限、线程数 | parity | [`ReadExtendedInfo`](../witr/internal/proc/extended_linux.go)、[`ReadExtendedInfo（Windows）`](../witr/internal/proc/extended_windows.go)、[`ReadExtendedInfo（macOS）`](../witr/internal/proc/extended_darwin.go)、[`ReadExtendedInfo（FreeBSD）`](../witr/internal/proc/extended_freebsd.go) | platform + ui 进程详情（总计划·固定决策「保留资源」） | Linux /proc statm+io+fd+status；macOS libproc cgo；Windows PSAPI+IO_COUNTERS；FreeBSD ps/rss+vsz + ps -H + lsof |
| 资源上下文：CPU 使用率、内存用量、平台相关热状态、阻止睡眠与挂起状态 | parity | [`GetResourceContext / GetCPUPercent`](../witr/internal/proc/resource_linux.go)、[`GetResourceContext（macOS）`](../witr/internal/proc/resource_darwin.go)、[`GetResourceContext（Windows）`](../witr/internal/proc/resource_windows.go)、[`GetResourceContext（FreeBSD）`](../witr/internal/proc/resource_freebsd.go)、[`TestGetResourceContextSelf / TestGetResourceContextNonexistentPID`](../witr/internal/proc/resource_windows_test.go) | platform + ui 进程详情（总计划·固定决策「保留资源」） | Linux ps/top + /sys/class/thermal + logind D-Bus；macOS pmset assertions/therm；Windows 仅 CPU/内存；任一失败返回 nil |
| 阻止睡眠检测 | parity | [`checkPreventsSleep（2s 超时，D-Bus 调用）`](../witr/internal/proc/resource_linux.go)、[`checkPreventsSleep（macOS，containsWholeWord 匹配 PID）`](../witr/internal/proc/resource_darwin.go) | platform（总计划·固定决策「保留资源」） | Linux 依赖 systemd-logind 总线可用，否则 false；macOS 依赖 pmset；Windows/FreeBSD 无 |
| CPU 使用率补充采集 | intentional change | [`GetCPUPercent / energyImpactLabel / getThermalState / getAppNapped`](../witr/internal/proc/resource_linux.go) | platform（总计划·固定决策「保留资源」；CPU 呈现将改为两样本差分） | 仅 Linux（依赖 top/ps 与 /sys/class/thermal thermal_zone0）；macOS 用 ps，Windows 用 GetProcessTimes |
| 时间格式化 | intentional change | [`humanDuration、formatRelativeTime、usecToTime`](../witr/internal/source/systemd_linux.go) | core 工具函数 + ui 展示 | systemd 附属；0 与 uint64 最大值（systemd 的 n/a 哨兵）都视为无值 |

## 2. 五类调查目标与目标解析

| 行为或能力 | 状态 | witr 证据 | Runquiry 对应模块 | 平台差异或说明 |
|---|---|---|---|---|
| 目标共五类：名称、PID、端口、文件、容器，由 `model.TargetType` 常量枚举（name/pid/port/file/container） | parity | [`TargetType / TargetName / TargetPID / TargetPort / TargetFile / TargetContainer`](../witr/pkg/model/target.go) | core 目标解析（总计划·公共领域类型 QueryTarget） | 无 |
| 五类目标类型被完整映射为 `QueryTarget` 枚举（`ProcessName{query,exact}`、`Pid`、`Port`、`File`、`Container{query,exact}`），其中 Name 与 Container 携带 exact 开关 | parity | [`Resolve(t model.Target, exact bool)`](../witr/internal/target/resolve.go) 与上级方案 [QueryTarget 定义](../.omo/plans/runquiry-gpui-desktop.md) | core 目标解析（总计划·公共领域类型）+ ui 调查入口 | 无 |
| PID 目标必须能被 `strconv.Atoi` 解析为整数，否则报 invalid pid | parity | [`Resolve（case model.TargetPID）`](../witr/internal/target/resolve.go) 与 [`TestResolvePIDWithExactFlag`](../witr/internal/target/resolve_test.go) | core 目标解析 | 无 |
| PID 目标必须为正整数（pid<=0 报 invalid pid: must be a positive integer），解析成功直接返回单元素 PID 列表，不做存活校验 | parity | [`Resolve（if pid <= 0）`](../witr/internal/target/resolve.go) 与 [`TestResolvePIDWithExactFlag`](../witr/internal/target/resolve_test.go) | core 目标解析 | 无 |
| 端口目标必须在 1-65535 区间，否则报 `invalid port: must be between 1 and 65535`，通过后委托 ResolvePort | parity | [`case model.TargetPort, port < 1 || port > 65535`](../witr/internal/target/resolve.go) | core 目标解析 | 无 |
| PID 与端口目标忽略 exact 标志，exact 仅对 Name 与 Container 生效 | parity | [`Resolve（PID/Port 分支不使用 exact）`](../witr/internal/target/resolve.go) 与 [`TestResolvePIDWithExactFlag / TestResolvePortWithExactFlag`](../witr/internal/target/resolve_test.go) | core 目标解析（仅 Name/Container 提供 exact 开关） | 无 |
| File 目标忽略 exact 标志，直接按路径解析 | parity | [`case model.TargetFile: return ResolveFile(val)`](../witr/internal/target/resolve.go) | core 目标解析 + platform FileInventory | Windows 上 File 目标为 Unsupported（见 file 条目） |
| 未知目标类型（包括 Resolve 内未实现的 container）返回 unknown target 错误；容器目标由调用方在进入 Resolve 之前分流处理 | intentional change | [`default: return nil, fmt.Errorf("unknown target")`](../witr/internal/target/resolve.go) 与 [`processTarget（t.Type == model.TargetContainer 时走 processContainerTarget）`](../witr/internal/app/app.go) | core 目标解析（QueryTarget::Container）；容器解析改由 platform ContainerInventory 提供 | 无 |
| 名称解析大小写不敏感：查询串与 comm/cmdline 均转小写后比较 | parity | [`ResolveName（lowerName := strings.ToLower(name)）`](../witr/internal/target/name_linux.go)、[`name_darwin.go`](../witr/internal/target/name_darwin.go)、[`name_windows.go`](../witr/internal/target/name_windows.go) | core 目标解析 | 三平台一致，均为小写化比较 |
| 名称匹配先比对进程 comm（可执行名），未命中再比对完整命令行 | parity | [`ResolveName（/proc/<pid>/comm 与 /proc/<pid>/cmdline 两段）`](../witr/internal/target/name_linux.go)、[`ResolveName（comm 与 args 两段）`](../witr/internal/target/name_darwin.go) | core 目标解析 | Windows 第一遍只比可执行名（ToolHelp32 快照），仅当无命中才进行第二遍 PEB cmdline 读取 |
| exact=true 时 comm 需完全相等、cmdline 需整词 token 匹配；exact=false 时为大小写不敏感的子串包含匹配 | parity | [`TestResolveWithExactFlag`](../witr/internal/target/resolve_test.go) | core 目标解析（Name exact 开关） | 无 |
| exact 的 cmdline 匹配按完整 token/路径段匹配：允许匹配完整参数、路径末段或中间路径段（如 core24 匹配 /snap/core24/1349/bin/python），不允许 token 内子串（foo 不匹配 /usr/local/bin/foo-bar）；正反斜杠都视为路径分隔符 | parity | [`matchesExactToken`](../witr/internal/target/resolve.go) 与 [`TestMatchesExactToken`（8 个用例含 Windows 反斜杠路径）](../witr/internal/target/resolve_test.go) | core 目标解析 | 反斜杠归一化专为 Windows 风格路径设计，Linux/macOS/FreeBSD 同样生效 |
| 名称解析排除自身及其祖先进程链（shell、sudo 等），避免 witr bash 总是命中用户自己的 shell | parity | [`isIgnored 闭包 + procpkg.ResolveAncestry`](../witr/internal/target/name_linux.go) | core 目标解析 | Linux 惰性构建 ignored 集合；macOS/FreeBSD/Windows 在扫描前即构建 |
| 名称解析禁止把纯数字 PID 字符串当作进程名命中对应进程（查询值等于某 PID 的十进制字符串时跳过该进程） | parity | [`lowerName == strconv.Itoa(pid) 时 continue`](../witr/internal/target/name_linux.go) | core 目标解析 | Linux/macOS/FreeBSD 实现；Windows 版无此防护 |
| Linux 名称解析在 /proc 扫描零命中时回退到 systemd：接受 foo 与 foo.service 两种写法，用 systemctl show -p MainPID --value 取主进程 PID，PID 为 0 视为服务未运行 | parity | [`resolveSystemdServiceMainPID / ResolveName（len(procPIDs) == 0 时才调用）`](../witr/internal/target/name_linux.go) | core 目标解析 + platform 服务来源 systemd | 仅 Linux |
| macOS 名称解析通过 `ps -axo pid=,comm=,args=` 枚举进程，零命中后回退 launchd：先校验 label（`^[a-zA-Z0-9._-]+$`，长度 1-256）再依次尝试 name、com.apple.name、org.name、io.name 四种 label，从 launchctl print 输出解析 "pid = <n>" | out of scope | [`ResolveName / resolveLaunchdServicePID / isValidServiceLabel / validServiceLabelRegex`](../witr/internal/target/name_darwin.go) | 无（macOS 支持已移出 v1 范围；app 的 launchd 名称解析回退已删除） | 仅 macOS |
| launchd/rc.d 服务 label 输入校验用于防命令注入：只允许字母数字与 `. _ -`，长度 1-256（FreeBSD 版要求首字符为字母数字） | out of scope | [`isValidServiceLabel / validServiceLabelRegex`](../witr/internal/target/name_darwin.go)、[`name_freebsd.go`](../witr/internal/target/name_freebsd.go) | 无（macOS 与 FreeBSD 均已移出 v1 范围） | macOS 与 FreeBSD 正则略有差异（FreeBSD 要求首字符为字母数字） |
| 名称解析结果去重并按 PID 升序排序，服务解析出的 PID 排在首位且与进程扫描结果去重 | parity | [`seen map + sort.Ints，servicePID 前置`](../witr/internal/target/name_linux.go)、[`ResolveName（同逻辑）`](../witr/internal/target/name_darwin.go)、[`name_freebsd.go（同逻辑）`](../witr/internal/target/name_freebsd.go) | core 目标解析（多结果进入候选表） | Linux/macOS/FreeBSD 一致；Windows 版不做排序/服务前置 |
| 名称解析无结果时返回明确错误：Unix 为 `no running process or service named %q`，Windows 为 `no process found matching: %s` | parity | [`ResolveName（len(pids)==0 分支）`](../witr/internal/target/name_linux.go)、[`name_darwin.go`](../witr/internal/target/name_darwin.go)、[`name_freebsd.go`](../witr/internal/target/name_freebsd.go)、[`name_windows.go（no process found matching: %s）`](../witr/internal/target/name_windows.go) | core InspectError::NotFound | Linux/macOS/FreeBSD 与 Windows 错误文案不同 |
| Windows 名称解析采用两遍策略：第一遍 ToolHelp32 快照对可执行名做即时匹配，仅当零命中才对存活候选逐个读 PEB 取命令行，SYSTEM 进程读取被拒时静默跳过 | parity | [`procpkg.ListProcessSnapshot / procpkg.GetProcessDetailedInfo，pass 1/pass 2 注释`](../witr/internal/target/name_windows.go) | platform 深度信息 windows crate、PEB + core 目标解析 | 仅 Windows |
| FreeBSD 名称解析用 `ps -axww -o pid -o comm -o args`（跳过表头），服务回退读 `/var/run/<name>.pid` 并用 ps 验证进程存活，再回退 service <name> status 输出解析 pid | out of scope | [`ResolveName / resolveRcServicePID`](../witr/internal/target/name_freebsd.go) | 总计划明确 FreeBSD jail 不实现，平台矩阵只含 Linux/Windows | 仅 FreeBSD |
| 文件目标解析前先取绝对路径，并尝试 EvalSymlinks 归一化（失败则退回绝对路径） | parity | [`filepath.Abs + filepath.EvalSymlinks`](../witr/internal/target/file_linux.go) | core 目标解析 File(PathBuf)；ui 调查入口 File 支持手工路径与文件选择器 | Linux 做符号链接归一化；macOS/Windows/FreeBSD 只取绝对路径 |
| Linux 文件解析遍历 `/proc/<pid>/fd` 的 readlink 结果，与归一化路径或绝对路径任一相等即命中，每个进程最多计一次 | parity | [`linkPath == realPath || linkPath == absPath，break`](../witr/internal/target/file_linux.go) | platform FileInventory /proc locks、FD | 仅 Linux |
| macOS 文件解析用 `lsof -F p <absPath>`，lsof 退出码 1 视为无进程持有，其他失败报 lsof failed | out of scope | [`ResolveFile（exec.Command("lsof", "-F", "p", absPath)，ExitError.ExitCode()==1 分支）`](../witr/internal/target/file_darwin.go) | 无（macOS 支持已移出 v1 范围） | 仅 macOS |
| Windows 文件解析用 Restart Manager（rstrtmgr.dll 的 RmStartSession/RmRegisterResources/RmGetList/RmEndSession），先 sizing 再取列表，RM_PROCESS_INFO 结构体必须精确 668 字节 | intentional change | [`ResolveFile / rmProcessInfo / rmUniqueProcess`](../witr/internal/target/file_windows.go) 与 [`TestRmProcessInfoSize`](../witr/internal/target/file_windows_test.go) | 总计划平台矩阵将 Windows 文件锁定为 Unsupported，File Locks 页保留入口并展示 Unsupported 说明 | 仅 Windows |
| FreeBSD 文件解析用 fstat <absPath> 输出第三列作为 PID，无命中报 no process found holding file | out of scope | [`ResolveFile`](../witr/internal/target/file_freebsd.go) | 总计划平台矩阵不含 FreeBSD | 仅 FreeBSD |
| 文件目标无进程持有时返回错误：Linux/macOS/FreeBSD 为 `no process found holding file: <path>`，Windows 为 `no process is holding <path>` | parity | [`ResolveFile`](../witr/internal/target/file_linux.go)、[`file_darwin.go`](../witr/internal/target/file_darwin.go)、[`file_freebsd.go`](../witr/internal/target/file_freebsd.go)、[`file_windows.go（needed == 0 / len(pids)==0 分支）`](../witr/internal/target/file_windows.go) | core InspectError::NotFound | Windows 文案不同 |
| 解析器返回哨兵错误 `ErrSocketOwnerUnknown` 与 `ErrUnsupported`，供调用方用 errors.Is 分支而非匹配文案；包装后仍可识别且两哨兵互相独立 | parity | [`ErrSocketOwnerUnknown / ErrUnsupported`](../witr/internal/target/errors.go) 与 [`TestSentinelErrors`](../witr/internal/target/errors_test.go) | core InspectError 枚举 | 无 |
| 平台不支持的目标（如 Windows 上的 -f）单独处理：不附加"换个名称/端口/PID 再试"的通用提示，直接输出错误并按无效输入退出码返回 | parity | [`errors.Is(err, target.ErrUnsupported) 分支注释`](../witr/internal/app/app.go) | core InspectError::Unsupported + ui 各工作区 Unsupported 展示 | 典型场景为 Windows 上不支持 file 目标 |
| 目标解析得到多个 PID 时不自动选择，而是打印候选列表并提示改用 `witr --pid <pid>`，以无效输入退出码结束（--env 场景提示 `witr --pid <pid> --env`） | intentional change | [`processTarget（len(pids) > 1 分支调用 printMultiMatch）`](../witr/internal/app/app.go)、[`processEnvTarget（len(pids) > 1 分支）`](../witr/internal/app/app.go)、[`multimatch_test.go（多匹配测试文件）`](../witr/internal/app/multimatch_test.go) | ui 调查入口（多结果进入候选表，不自动选择第一项）；呈现形式从 CLI 提示改为候选表属有意变更 | 无 |
| 容器目标在所有已注册且可用的运行时中查询，按 runtime\|id 去重；匹配字段为容器名、镜像、命令、compose project、compose service（小写比较，exact 为全等否则子串包含） | parity | [`ResolveContainer / matchContainer（seen key = rt.Name()+"|"+c.ID）`](../witr/internal/proc/container_runtime.go) | platform ContainerInventory（容器按 runtime + id 去重）+ core 目标解析 `Container{query, exact}` | 无（运行时集合取决于主机实际存在的 CLI） |
| 容器目标无结果报 `no container found matching %q`；多结果不自动选择并按无效输入退出码返回 | parity | [`processContainerTarget（len(matches)==0 / len(matches)>1 分支，multiple containers matched）`](../witr/internal/app/app.go) | core InspectError::NotFound/Ambiguous + ui 调查入口（多结果进入候选表） | 无 |
| 容器目标解析到唯一容器后先富集信息再取主机 PID；仅当主机 PID 存在且经 PIDBelongsToContainer 验证确实属于该容器时走完整进程分析，否则降级为容器自身的 fallback 展示 | parity | [`processContainerTarget（procpkg.ResolveContainerHostPID + procpkg.PIDBelongsToContainer 分支）`](../witr/internal/app/app.go)、[`ResolveContainerHostPID`](../witr/internal/proc/container_runtime.go) 与 [`PIDBelongsToContainer`](../witr/internal/proc/container_verify_linux.go) | core + platform（Containers 页 无主机 PID 时展示容器自身的部分结果） | 无（依赖各运行时 CLI 是否能给出主机 PID） |
| 端口解析出 socket 但属主不可知（ErrSocketOwnerUnknown）时，若目标为端口则回退按端口查 Docker 容器（docker ps --filter publish=<port> --no-trunc），命中即渲染容器 fallback 结果而非报错 | parity | [`handleResolveError（ErrSocketOwnerUnknown 分支调用 procpkg.ResolveContainerByPort）`](../witr/internal/app/app.go) 与 [`ResolveContainerByPort`](../witr/internal/proc/container.go) | platform ContainerInventory + core 来源 Container（容器 CLI 走 CommandRunner） | Linux/macOS/Windows 均走该回退；ResolveContainerByPort 本身要求 docker 在 PATH 中 |
| 端口解析到 PID 1 且 systemd 在运行时，进一步按端口号解析 systemd 服务名并把结果 ResolvedTarget 覆盖为服务名（去掉 .service 后缀） | parity | [`processTarget（t.Type == model.TargetPort && pid == 1 && source.IsSystemdRunning() 分支，res.ResolvedTarget 赋值）`](../witr/internal/app/app.go) | core 目标解析/来源识别 systemd | 仅 Linux（systemd） |
| 解析结果为空列表（err 为 nil 且 len(pids)==0）时统一视为 no matching process found 错误进入错误处理 | parity | [`processTarget（if err == nil && len(pids) == 0）`](../witr/internal/app/app.go) | core InspectError::NotFound | 无 |
| 文件目标解析失败且当前用户非 root 时，错误信息追加建议用 sudo 重试的提示（仅非 Windows 且 euid!=0） | intentional change | [`handleResolveError（t.Type == model.TargetFile && runtime.GOOS != "windows" && os.Geteuid() != 0 分支）`](../witr/internal/app/app.go) | ui/app 错误呈现（应用不自动提权） | Linux/macOS（依赖 os.Geteuid）；Windows 不适用 |
| socket 属主不可知且容器回退未命中时，提示可能权限不足并给出 sudo 命令，按权限错误退出码返回 | intentional change | [`handleResolveError（A socket was found for the port... Try running with sudo，return ExitPermission）`](../witr/internal/app/app.go) | ui/app 错误呈现 | 无（文案通用，触发条件各平台均为 ErrSocketOwnerUnknown） |
| 名称解析直接读进程表而不经过 ps\|grep 管道，因此 grep 本身也可被按名解析（曾存在对 grep 的静默过滤，已移除并加回归测试） | parity | [`TestIntegration_ResolveNameFindsGrep`](../witr/internal/target/integration_test.go) | core 目标解析（必测场景 五类目标的多结果/无结果） | 非 Windows 平台执行，Windows 跳过 |
| 名称/端口/文件解析器与真实 OS 状态端到端联测：spawn 子进程可被按名找到、回环 TCP 监听可归因到本进程、无主端口必须报错、自建文件可归因到本进程（macOS/FreeBSD 文件路径容错跳过） | parity | [`TestIntegration_ResolveNameFindsSpawnedChild / TestIntegration_ResolvePortFindsLoopbackListener / TestIntegration_ResolvePortNonexistent / TestIntegration_ResolveFileSelf`](../witr/internal/target/integration_test.go) | core/platform 必测场景 | 全平台运行；macOS/FreeBSD 的 ResolveFile 归因失败被容错跳过 |
## 3. 目标解析与多结果行为（补充）

| 行为或能力 | 状态 | witr 证据 | Runquiry 对应模块 | 平台差异或说明 |
|---|---|---|---|---|
| 空目标校验：既无 `--pid/--port/--file/--container/--env` 也无位置参数以外的目标时报错 `must specify --pid, --port, --file, --container, or a process name`，退出码 4 | intentional change | [`runApp（len(targets)==0 分支）与 ExitInvalidInput`](../witr/internal/app/app.go) | core 目标解析 / ui 调查栏 | 无 |
| 多目标按命令行顺序逐一处理，单目标失败（解析错误/未找到）不中断其余目标 | intentional change | [`runApp、processTarget`](../witr/internal/app/app.go) | core 目标解析 + app | 无 |
| 多目标 --json 时把每个目标的结果字符串缩进后包装成顶层数组输出；失败目标以 `{Target, Error}` 条目占位而不是丢弃，多个结果间用逗号分隔 | intentional change | [`runApp（jsonResults 拼装）、jsonErrorEntry`](../witr/internal/app/app.go) 与 [`TestJSONErrorEntry`](../witr/internal/app/collect_test.go) | core 部分成功语义；JSON 数组序列化格式本身不保留 | 无 |
| 退出码契约：0 成功、1 有告警、2 未找到、3 权限、4 非法输入、5 内部错误；多目标最终退出码取各目标中的最高值 | intentional change | [`ExitOK..ExitInternalError 常量、withExitCode、runApp 的 highestExit`](../witr/internal/app/app.go) 与 [`TestExitCodes`](../witr/internal/app/exitcode_test.go) | core 错误分类；数字退出码本身不进入 GUI | 无 |
| 错误到退出码的映射靠错误字符串嗅探：classifyError 按小写消息中的 permission denied / not found / invalid 等子串归类，未命中归为内部错误 5 | intentional change | [`classifyError`](../witr/internal/app/app.go) 与 [`TestClassifyError`](../witr/internal/app/app_test.go) | core 错误分类（不再做字符串嗅探） | 无 |
| 名称/端口解析命中多个 PID 时不自动选择：列出候选（序号、命令、pid、cmdline）并提示改用 `witr --pid <pid>`，退出码 4 | parity | 同 §2 多结果条：[`processTarget（len(pids)>1 分支）、printMultiMatch`](../witr/internal/app/app.go) 与 [`TestPrintMultiMatch`](../witr/internal/app/multimatch_test.go) | core 目标解析 + ui 候选表 | 「不自动选择、列候选」语义为 parity；呈现形式从 CLI 提示改为 GPUI 候选表（见 §2 的 intentional change 说明） |
| 容器目标命中多个容器时同样列出候选（名称、runtime、image、status、ports）并提示 `witr -c <container-name> --exact`，退出码 4；所有展示字段先经 SanitizeTerminal 净化 | parity | [`processContainerTarget、printContainerMultiMatch`](../witr/internal/app/app.go) 与 [`TestPrintContainerMultiMatch`](../witr/internal/app/multimatch_test.go) | core 容器目标解析 / ui 容器工作区候选表 | 无 |
| 容器匹配谓词：跨 name/image/command/compose project/compose service 五个字段做不区分大小写子串匹配；--exact 时要求与任一字段完全相等；空字段跳过 | parity | [`matchContainer / ResolveContainer`](../witr/internal/proc/container_runtime.go) | core 目标解析（QueryTarget.Container{query, exact}） | 无 |
| 容器结果跨运行时去重与合并：按 `runtime\|containerID` 去重（seen map），ResolveContainer 只查 Available() 的运行时；ListAllContainers 供 TUI 容器标签页全量列出 | parity | [`ResolveContainer / ListAllContainers / registeredRuntimes`](../witr/internal/proc/container_runtime.go) | platform 容器运行时 | 无 |
| 主机 PID 不可用或不属于该容器时，改用容器运行时侧元数据渲染 fallback（standard/short/tree/warnings/JSON 五种变体），仍返回退出码 0 | parity | [`processContainerTarget 末尾 switch`](../witr/internal/app/app.go) 与 [`RenderContainerFallback / RenderContainerFallbackShort / RenderContainerFallbackTree / RenderContainerFallbackWarnings / ContainerFallbackToJSON`](../witr/internal/output/docker.go) | ui Containers 工作区 | 非 Linux 上因 PIDBelongsToContainer 恒为 false 而总是走 fallback |
| SocketInfo 附带人类可读解释与排查建议（Explanation/Workaround） | out of scope | 同 §2 对应条：[`SocketInfo.Explanation` 与 `SocketInfo.Workaround`](../witr/pkg/model/socket.go) | ui 文案 | 状态名为 Linux TCP 状态集 |
| 进程内 Socket 条目字段（Inode、Port、Address、State、Protocol） | parity | 同 §2 对应条：[`Socket{Inode, Port, Address, State, Protocol}`](../witr/pkg/model/socket.go) | core 领域模型 + ui Ports 工作区 | Inode 仅 Linux 有意义 |
| 端口查询的 Socket 状态快照（Port、State、LocalAddr、RemoteAddr） | parity | 同 §2 对应条：[`SocketInfo{Port, State, LocalAddr, RemoteAddr}`](../witr/pkg/model/socket.go) | core 领域模型 + ui Ports 工作区 | 无 |
| 开放端口条目（PID、Port、Address、Protocol、State） | parity | 同 §2 对应条：[`OpenPort{PID, Port, Address, Protocol, State}`](../witr/pkg/model/net.go) | core 领域模型 + ui Ports 工作区 | Linux /proc/net + FD inode；macOS lsof -F；Windows IP Helper API |
| 文件目标解析前先取绝对路径，并尝试 EvalSymlinks 归一化（失败则退回绝对路径） | parity | 同 §2 对应条：[`filepath.Abs + filepath.EvalSymlinks`](../witr/internal/target/file_linux.go) | core 目标解析 File(PathBuf) | Linux 做符号链接归一化 |
| Linux 文件解析遍历 `/proc/<pid>/fd` 的 readlink 结果，与归一化路径或绝对路径任一相等即命中，每个进程最多计一次 | parity | 同 §2 对应条：[`linkPath == realPath || linkPath == absPath，break`](../witr/internal/target/file_linux.go) | platform FileInventory /proc locks、FD | 仅 Linux |
| macOS 文件解析用 `lsof -F p <absPath>`，lsof 退出码 1 视为无进程持有，其他失败报 lsof failed | out of scope | 同 §2 对应条：[`ResolveFile（exec.Command("lsof", "-F", "p", absPath)，ExitError.ExitCode()==1 分支）`](../witr/internal/target/file_darwin.go) | 无（macOS 支持已移出 v1 范围） | 仅 macOS |
| Windows 文件解析用 Restart Manager（rstrtmgr.dll），先 sizing 再取列表，RM_PROCESS_INFO 结构体必须精确 668 字节 | intentional change | 同 §2 对应条：[`ResolveFile / rmProcessInfo / rmUniqueProcess`](../witr/internal/target/file_windows.go) 与 [`TestRmProcessInfoSize`](../witr/internal/target/file_windows_test.go) | 总计划平台矩阵将 Windows 文件锁定为 Unsupported | 仅 Windows |
| 平台不支持的目标（如 Windows 上的 -f）单独处理：不附加"换个名称/端口/PID 再试"的通用提示，直接输出错误并按无效输入退出码返回 | parity | 同 §2 对应条：[`errors.Is(err, target.ErrUnsupported) 分支注释`](../witr/internal/app/app.go) | core InspectError::Unsupported + ui 各工作区 Unsupported 展示 | 典型场景为 Windows 上不支持 file 目标 |
| socket 属主不可知且容器回退未命中时，提示可能权限不足并给出 sudo 命令，按权限错误退出码返回 | intentional change | 同 §2 对应条：[`handleResolveError（A socket was found for the port... Try running with sudo，return ExitPermission）`](../witr/internal/app/app.go) | ui/app 错误呈现 | 无（文案通用，触发条件各平台均为 ErrSocketOwnerUnknown） |

## 4. 分析管线与进程采集

| 行为或能力 | 状态 | witr 证据 | Runquiry 对应模块 | 平台差异或说明 |
|---|---|---|---|---|
| 分析管线以 PID 为唯一输入，按固定顺序执行：祖先链解析 → 来源识别 → 目标进程选取 → 容器健康检查补全 → 子进程/扩展信息/资源/文件上下文收集 → 告警生成 → 组装 model.Result，由 AnalyzePID 单一入口完成 | parity | [`AnalyzePID` 与 `AnalyzeConfig（PID/Verbose/Tree/Target）`](../witr/internal/pipeline/analyze.go) | core 分析管线（02-core-analysis B1） | 无 |
| 祖先链从目标 PID 沿 PPID 逐跳上溯，直到 PPID==0 或 PID==1，最终按根→目标顺序返回；用已访问集合做环检测 | parity | [`ResolveAncestry`（seen map 注释 loop protection，chain 反转为 root first）](../witr/internal/proc/ancestry.go) | core 祖先链（固定决策「保留 witr 的祖先链」） | 无 |
| 部分成功（向上）：某一跳 ReadProcess 失败时祖先链就地截断而非整体失败，但若一跳都没读到则 ResolveAncestry 返回错误 | parity | [`ReadProcess 出 err 即 break；len(chain)==0 返回 error`](../witr/internal/proc/ancestry.go) 与 [`TestAnalyzePID_Nonexistent`](../witr/internal/pipeline/analyze_test.go) | core 部分成功语义（Inspection\<T\>） | 无 |
| 目标进程取祖先链最后一项，ResolvedTarget 取该进程的 Command；链为空时 ResolvedTarget 回退为字符串 "unknown" | intentional change | [`proc = ancestry[len(ancestry)-1]` 与 `resolvedTarget = proc.Command / "unknown"`](../witr/internal/pipeline/analyze.go)、[`Result.ResolvedTarget`](../witr/pkg/model/result.go)、[`TestAnalyzePID_Self`](../witr/internal/pipeline/analyze_test.go) | core 目标解析 | 无 |
| witr 的目标只有 Type+Value 两个字符串（name/pid/port/file/container 五种 TargetType）；Runquiry 改为显式 QueryTarget 枚举，不自动猜测 | intentional change | [`Target` 与 `TargetType`](../witr/pkg/model/target.go)（`AnalyzeConfig.Target` 直接透传到 `model.Result.Target`） | core 目标解析（总计划·公共领域类型 QueryTarget、调查入口「显式目标类型，不自动猜测」） | 无 |
| 来源识别按固定优先级链依次尝试：container > ssh > shell > systemd > launchd > BSD rc > supervisor > cron > Windows service > init，命中即返回 | parity | [`Detect`（注释：Detection order prioritizes platform-specific init systems over generic supervisor detection）](../witr/internal/source/detect.go) | core 来源识别（总计划·来源至少覆盖九类 + 兜底） | 各来源探测按平台分文件实现（launchd_darwin.go、bsdrc_freebsd.go、systemd_linux.go、service_windows.go 等）；优先级顺序跨平台一致 |
| 来源识别永不返回空 | parity | [`Detect` 末尾 `return model.Source{Type: model.SourceUnknown}`](../witr/internal/source/detect.go) 与 [`TestAnalyzePID_Self`（断言 Source.Type 非空，注释：never blank）](../witr/internal/pipeline/analyze_test.go) | core 来源识别（总计划·来源识别与告警） | 无 |
| 容器标签一致性改写：祖先命令为 incus/lxd/lxc 时，把由 ReadProcess 写入的通用标签 "lxc-based:"/"lxc-based" 重写为具体 runtime 名前缀，使 Container 行与 Source 行一致 | parity | [`AnalyzePID 的 src.Type == model.SourceContainer 分支`（注释解释 ReadProcess 看不到 ancestry 只能标 lxc-based）](../witr/internal/pipeline/analyze.go)、[`detectContainer`](../witr/internal/source/container.go) | core 容器上下文 | 依赖 Linux cgroup 容器检测；容器运行时范围含 Incus/LXC/LXD |
| 容器健康检查补全 | parity | [`ContainerHealthcheckStatus`](../witr/internal/proc/container.go)、[`ContainerHealthcheck` 字段注释（`""`, `"present"`, or `"absent"`）](../witr/pkg/model/process.go) | core 容器上下文 + 告警规则 | 健康检查探测仅对 docker/podman 生效；其余运行时/平台不可判定 |
| 子进程收集：取一次全量进程快照，筛出 PPID 等于目标的子进程，按 PID 升序排序，同一份结果同时填充 Result.Children 与 proc.Children | parity | [`cfg.Verbose \|\| cfg.Tree 分支与 sort.Slice/sort.Ints`](../witr/internal/pipeline/analyze.go)、[`TestAnalyzePID_Verbose`](../witr/internal/pipeline/analyze_test.go)、[`Children []int` 字段（Extended information for verbose output）](../witr/pkg/model/process.go) | core 分析管线 + ui 进程详情（总计划·页面 Processes「祖先树」） | ListProcessSnapshot 在 Linux/macOS/FreeBSD/Windows 各有实现 |
| 子进程快照失败时静默降级：ListProcessSnapshot 返回错误则 childPIDs/childProcesses 保持空，不影响其余分析结果继续返回 | parity | [`if snapshot, err := procpkg.ListProcessSnapshot(); err == nil 分支`](../witr/internal/pipeline/analyze.go) | core 部分成功语义（公共领域类型 Inspection\<T\>） | 无 |
| 仅 Verbose 模式读取扩展信息并填充：Memory、IO、FileDescs、FDCount、FDLimit、ThreadCount、Children；ReadExtendedInfo 出错则这些字段整体保持零值，不部分填充 | parity | [`cfg.Verbose && len(ancestry) > 0 分支调用 procpkg.ReadExtendedInfo`](../witr/internal/pipeline/analyze.go)、[`ReadExtendedInfo（返回 memInfo, ioStats, fileDescs, fdCount, fdLimit, threadCount, err）`](../witr/internal/proc/extended_linux.go)、[`Memory/IO/FileDescs/FDCount/FDLimit/Children/ThreadCount` 字段（json omitempty，verbose 专用）](../witr/pkg/model/process.go) | core 分析管线 + ui 进程详情（总计划·页面 Processes） | ReadExtendedInfo 有 Linux/macOS/FreeBSD/Windows 四份实现 |
| ResourceContext 与 FileContext 仅在 Verbose 模式收集，均为指针类型，可为 nil 表示『未收集/不可得』；FileContext 收集失败返回 nil | parity | [`cfg.Verbose 分支调用 procpkg.GetResourceContext 与 procpkg.GetFileContext`](../witr/internal/pipeline/analyze.go)、[`ResourceContext/FileContext` 指针字段](../witr/pkg/model/result.go)、[`GetFileContext` 注释 Will return nil if the context could not be gathered](../witr/internal/proc/filecontext_linux.go) | core 分析管线 + platform 采集（能力矩阵「深度信息」「文件锁/打开文件」） | GetResourceContext/GetFileContext 均有 linux/darwin/freebsd/windows 四份实现 |
| RestartCount 仅在来源为 systemd 时从 src.Details["NRestarts"] 解析为整数 | parity | [`src.Type == model.SourceSystemd 分支（strconv.Atoi 解析 Details["NRestarts"]）`](../witr/internal/pipeline/analyze.go)、[`Result.RestartCount`](../witr/pkg/model/result.go) | core 分析管线（总计划·来源识别与告警） | 仅 systemd 来源（Linux）；launchd/BSD rc/Windows SCM 等无 NRestarts 概念 |
| 告警由 source.Warnings(ancestry, restartCount, srcType) 生成，仅基于链尾（目标）进程判定，返回有序字符串列表；调用方传入已解析的来源类型避免重复探测 | parity | [`Warnings`（last := p[len(p)-1]；len(p)==0 返回 nil；srcType 可变参数优先于重新 Detect）](../witr/internal/source/detect.go)、[`Warnings: source.Warnings(ancestry, restartCount, src.Type)`](../witr/internal/pipeline/analyze.go) | core 告警规则（总计划·来源识别与告警『从 witr 分析管线移植为纯函数』） | 无 |
| 告警顺序固定 | parity | [`Warnings` 函数体各 append 顺序](../witr/internal/source/detect.go)、[`Health` 字段注释（healthy/zombie/stopped/high-cpu/high-mem）](../witr/pkg/model/process.go) | core 告警规则 | 无 |
| 服务重启告警 | parity | [`if restartCount > 5 分支`（注释：real count from the service manager, or 0 when unknown）](../witr/internal/source/detect.go) | core 告警规则 | 仅 systemd 提供 NRestarts；其他平台来源该计数恒为 0 |
| 高负载与健康告警 | parity | [`switch last.Health` 分支（"Process is using high CPU (>2h total)" / "high memory (>1GB RSS)"）](../witr/internal/source/detect.go) | core 告警规则 | 健康标签由各平台 ReadProcess 计算 |
| 公开监听告警 | parity | [`IsPublicBind(last.Sockets)` 调用](../witr/internal/source/network.go) | core 告警规则 + ui Ports 工作区 | socket 列表采集按平台不同 |
| root 告警与危险 capabilities 告警互斥 | parity | [`dangerousCapabilities map` 与 `isDangerousCapability`、`Warnings` 的 `last.User == "root" / else if len(last.Capabilities) > 0` 分支](../witr/internal/source/detect.go) | core 告警规则 | capabilities 为 Linux 特有概念 |
| 未知来源告警 | parity | [`if st == model.SourceUnknown && runtime.GOOS != "windows"` 分支及其注释](../witr/internal/source/detect.go)、[`Warnings` 传入 `src.Type` 作为 `srcType`](../witr/internal/pipeline/analyze.go) | core 告警规则 | Windows 豁免；Linux/macOS/FreeBSD 照常告警 |
| 长运行告警 | parity | [`!last.StartedAt.IsZero() && time.Since(...).Hours() > 90*24` 分支及注释](../witr/internal/source/detect.go)、[`StartedAt` 字段](../witr/pkg/model/process.go) | core 告警规则 | 注释举例为 Windows 受保护进程；判定逻辑跨平台一致 |
| 可疑工作目录告警 | parity | [`suspiciousDirs map` 与 `Warnings` 的 `suspiciousDirs[last.WorkingDir]` 分支](../witr/internal/source/detect.go) | core 告警规则 | 路径为 Unix 风格 |
| 环境变量采集 | parity | [`ReadProcess` 读 /proc/\<pid\>/environ 按 \0 切分](../witr/internal/proc/process_linux.go)、[`getEnvironment（ps -E，SIP 受限）`](../witr/internal/proc/process_darwin.go)、[`readEnvironmentBlock / parseEnvBlock`](../witr/internal/proc/peb_windows.go)、[`getEnvironment（procstat -e）`](../witr/internal/proc/process_freebsd.go)、[`TestParseEnvBlock / TestReadProcessEnvSelf`](../witr/internal/proc/env_windows_test.go) | platform 采集 + ui 详情（总计划·安全交互「环境变量默认脱敏」） | Linux /proc/environ 读取最完整；macOS 受 SIP 限制常为空；Windows 需 PROCESS_VM_READ 读 PEB |
| 工作目录获取 | parity | [`os.Readlink /proc/<pid>/cwd`（失败置 unknown）](../witr/internal/proc/process_linux.go)、[`getCwdAndBinaryPath（lsof -d cwd,txt -F fn，非零退出但 stdout 有数据时抢救）`](../witr/internal/proc/process_darwin.go)、[`rtlUserProcessParameters.CurrentDirectoryPath / getFullProcessInfo`](../witr/internal/proc/peb_windows.go) | platform 采集（告警「可疑工作目录」依赖 cwd） | Linux readlink；macOS lsof -F；Windows 读 PEB RTL_USER_PROCESS_PARAMETERS，无 VM_READ 权限时 Cwd 置空 |
| I/O 统计 | parity | [`ReadExtendedInfo（返回 memInfo, ioStats, fileDescs, fdCount, fdLimit, threadCount, err）`](../witr/internal/proc/extended_linux.go) | core 领域模型 + ui 资源详情 | 无 |
| 打开文件描述符列表 | parity | [`Process.FileDescs []string`](../witr/pkg/model/process.go)、[`ReadExtendedInfo（Linux 侧填充 FileDescs）`](../witr/internal/proc/extended_linux.go) | core 领域模型 | 无 |

## 5. 来源识别及优先级

| 行为或能力 | 状态 | witr 证据 | Runquiry 对应模块 | 平台差异或说明 |
|---|---|---|---|---|
| 来源识别按固定优先级链依次尝试：container → ssh → shell → systemd → launchd → BSD rc → supervisor → cron → Windows Service → init，全部未命中则返回 SourceUnknown | parity | [`Detect`（依次调用 detectContainer/detectSSH/detectShell/detectSystemd/detectLaunchd/detectBsdRc/detectSupervisor/detectCron/detectWindowsService/detectInit，末尾返回 SourceUnknown）](../witr/internal/source/detect.go) | core 来源识别（总计划·来源至少覆盖九类 + 兜底） | 各平台 stub（launchd_freebsd.go、launchd_linux.go、launchd_windows.go、systemd_darwin.go、systemd_freebsd.go、systemd_windows.go、bsdrc_linux.go、bsdrc_darwin.go、bsdrc_windows.go、service_other.go）使不适用的来源恒为 nil |
| 容器识别：逐祖先读 /proc/PID/cgroup，按内容判定 docker/podman/libpod/kubepods/containerd/colima/lxc.payload 并回填 runtime 与容器 ID | parity | [`detectContainer`（switch 分支）](../witr/internal/source/container.go)、[`process_linux.go 的 cgroup switch 分支与 extractContainerID/extractLXCBasedContainerName`](../witr/internal/proc/process_linux.go) | core 来源识别 + platform Linux 采集 | 仅 Linux（cgroup）；Snap/Flatpak 环境变量分支读取目标进程 Env |
| LXC 系运行时动态判定：祖先命令为 incus/lxd/lxc-start 时动态判定 Incus/LXD/经典 LXC，无命中回退 "lxc" | parity | [`detectLXCRuntime`（switch + fallback 注释）](../witr/internal/source/container.go) | core 来源识别 | 无 |
| Snap/Flatpak 环境变量判定为容器来源（SNAP_NAME=/FLATPAK_ID=），且跳过容器健康检查告警 | parity | [`detectContainer` 的 SNAP_NAME=/FLATPAK_ID= 分支](../witr/internal/source/container.go) 与 [`TestWarningsSnapAndFlatpakSkipHealthcheck`](../witr/internal/source/warnings_test.go) | core 来源识别 | 主要见于 Linux 桌面 |
| SSH 来源判定：祖先链（排除目标自身）存在 sshd/sshd.exe 即判定，需 ancestry 长度 ≥2 | parity | [`detectSSH`（hasSSH 循环与 len(ancestry)>=2）](../witr/internal/source/ssh.go) | core 来源识别（SSH） | Windows 匹配 sshd.exe |
| SSH 连接详情从环境变量提取 | parity | [`detectSSH`（SSH_CLIENT/SSH_CONNECTION/SSH_TTY 回溯搜索）](../witr/internal/source/ssh.go) | core 来源识别 | su/sudo 清洗环境后仍可从祖先取到 |
| SSH 描述文本组装 | intentional change | [`detectSSH`（desc 组装段）](../witr/internal/source/ssh.go) | core 来源识别（详情描述字段） | 英文文案由 UI 呈现 |
| systemd 来源判定前提 | parity | [`IsSystemdRunning` 与 `detectSystemd`（hasPID1 检查）](../witr/internal/source/systemd_linux.go) | core 来源识别 + platform systemd | 仅 Linux；其他平台 stub 恒 false |
| systemd 单元名从 /proc/PID/cgroup 解析 | parity | [`getUnitNameFromCgroup`](../witr/internal/source/systemd_linux.go) | platform Linux 采集 | 仅 Linux（cgroup v1/v2） |
| systemd D-Bus 富化（Description/FragmentPath/SourcePath/NRestarts） | parity | [`enrichFromSystemd`（doc 注释 best-effort）](../witr/internal/source/systemd_linux.go) | platform Linux | 无 |
| systemd 定时器调度展示 | parity | [`timerSchedule、calendarSpec、monotonicSpec`](../witr/internal/source/systemd_linux.go) | platform Linux（详情面板 schedule 字段） | 无 |
| launchd 来源判定 | out of scope | [`detectLaunchd`（hasLaunchd 检查与 err 回退）](../witr/internal/source/launchd_darwin.go) | core 保留判定链（`detect_launchd`），platform macOS 已删除、无证据填充 | 仅 macOS |
| launchd 富化（label/comment/DomainDescription/PlistPath/FormatTriggers/KeepAlive） | out of scope | [`detectLaunchd` 各段](../witr/internal/source/launchd_darwin.go) | 无（platform macOS 已移出 v1 范围） | 无 |
| SSH 连接详情回溯 | parity | [`detectSSH`（for i := len-1 递减回溯）](../witr/internal/source/ssh.go) | core 来源识别 | 无 |
| Shell 来源判定 | parity | [`isShell`（名单含 bash/zsh/sh/fish/csh/tcsh/ksh/dash/ash/cmd.exe/powershell.exe/pwsh.exe/explorer.exe 等）](../witr/internal/source/shell.go) | core 来源识别（Shell） | Windows 下先剥离 .exe/.cmd/.bat/.com 后缀再查表 |
| Shell 来源识别用户工具 | parity | [`userTools`（python/node/ruby/perl/php/go/java/cargo/npm/yarn/make 等）](../witr/internal/source/shell.go) | core 来源识别（Shell） | Windows 下先剥离 .exe/.cmd/.bat/.com 后缀 |
| tmux/screen 多路复用器富化 | parity | [`enrichMultiplexer` 与 `findEnvVar`](../witr/internal/source/shell.go) | core 来源识别（详情描述） | 无 |
| findEnvVar 环境变量回溯 | parity | [`findEnvVar`（从目标向祖先回溯，返回首个命中值）](../witr/internal/source/shell.go) | core 来源识别 | 无 |
| cron 来源判定 | parity | [`detectCron`](../witr/internal/source/cron.go) | core 来源识别（cron） | Unix 守护进程名 |
| Windows 服务三级判定 | parity | [`detectWindowsService`（三级判定）](../witr/internal/source/service_windows.go) | core 来源识别 + platform Windows SCM | 仅 Windows |
| init 来源兜底 | parity | [`detectInit`（根进程 PID 1 / Windows PID 4 "System"）](../witr/internal/source/init.go) | core 来源识别（init） | Windows 特有 PID 4 |

## 6. 告警规则

| 行为或能力 | 状态 | witr 证据 | Runquiry 对应模块 | 平台差异或说明 |
|---|---|---|---|---|
| 告警由 source.Warnings(ancestry, restartCount, srcType...) 生成 | parity | 同 §4 告警生成条：[`Warnings（last := p[len(p)-1]；len(p)==0 返回 nil；srcType 可变参数优先于重新 Detect）`](../witr/internal/source/detect.go)、[`Warnings: source.Warnings(ancestry, restartCount, src.Type)`](../witr/internal/pipeline/analyze.go) | core 告警规则 | 无 |
| 服务重启告警 | parity | 同 §4 服务重启告警条：[`restartCount > 5 分支`](../witr/internal/source/detect.go) | core 告警规则 | 仅 systemd |
| 高负载与健康告警 | parity | 同 §4 高负载与健康告警条：[`switch last.Health`](../witr/internal/source/detect.go) | core 告警规则 | 健康标签由各平台 ReadProcess 计算 |
| 公开监听告警 | parity | 同 §4 公开监听告警条：[`IsPublicBind(last.Sockets)`](../witr/internal/source/network.go) | core 告警规则 + ui Ports 工作区 | socket 列表采集按平台不同 |
| root 告警与危险 capabilities 告警互斥 | parity | 同 §4 对应条：[`dangerousCapabilities` 与 `Warnings` 分支](../witr/internal/source/detect.go) | core 告警规则 | capabilities 为 Linux 特有概念 |
| 长运行告警 | parity | 同 §4 长运行告警条：[`time.Since(...).Hours() > 90*24` 分支](../witr/internal/source/detect.go) | core 告警规则 | 无 |
| 可疑工作目录告警 | parity | 同 §4 可疑工作目录告警条：[`suspiciousDirs[last.WorkingDir]` 分支](../witr/internal/source/detect.go) | core 告警规则 | 无 |
| 容器健康检查告警 | parity | [`ContainerHealthcheck == "absent"` 分支](../witr/internal/source/detect.go) | core 告警规则 | 健康检查探测仅 Linux 容器运行时可得 |
| 服务名与进程名不匹配告警 | parity | [`svcCore` 处理段](../witr/internal/source/detect.go) | core 告警规则 | 模板语法 @ 为 systemd 特有 |
| 已删除二进制告警 | parity | [`if last.ExeDeleted` 分支](../witr/internal/source/detect.go) | core 告警规则 | 字段采集随平台 |
| 可疑环境变量告警 | parity | [`envVarRules` 与 `envSuspiciousWarnings`](../witr/internal/source/detect.go) | core 告警规则 | DYLD_* 为 macOS 专属注入面 |
| 环境告警输出顺序确定性 | parity | [`envSuspiciousWarnings` 注释与 `sort.Strings(keys)`](../witr/internal/source/detect.go)、[`FuzzEnvSuspiciousWarningsDeterministic`](../witr/internal/source/detect_test.go) | core 告警规则（fixture 固定行为） | 无 |

## 7. 容器运行时

| 行为或能力 | 状态 | witr 证据 | Runquiry 对应模块 | 平台差异或说明 |
|---|---|---|---|---|
| 容器运行时范围：Docker、Podman、nerdctl（显示为 containerd）、crictl（显示为 k8s）、Incus、LXC、LXD；FreeBSD jail 不实现 | parity | [`registerRuntime` 与 `ContainerRuntime` 接口](../witr/internal/proc/container_runtime.go)，各运行时以 `init()` 注册：[`dockerRuntime`](../witr/internal/proc/runtime_docker.go)、[`podmanRuntime`](../witr/internal/proc/runtime_podman.go)、[`nerdctlRuntime`](../witr/internal/proc/runtime_nerdctl.go)、[`crictlRuntime`](../witr/internal/proc/runtime_crictl.go)、[`incusRuntime`](../witr/internal/proc/runtime_incus.go)、[`lxdRuntime`](../witr/internal/proc/runtime_lxd.go)、[`lxcRuntime`](../witr/internal/proc/runtime_lxc.go) | platform 容器运行时（总计划·容器运行时范围） | jail 仅 FreeBSD（见下条，Runquiry 不实现） |
| 容器识别：逐祖先读 /proc/PID/cgroup 按内容判定运行时 | parity | [`detectContainer`（cgroup 内容 switch 分支）](../witr/internal/source/container.go)、[`process_linux.go 的 cgroup 读取`](../witr/internal/proc/process_linux.go) | core + platform | 仅 Linux |
| 容器 ID 提取与截短（scope/路径两种 cgroup 模式、64 位十六进制长 ID、12 字符短 ID） | parity | [`extractContainerID`](../witr/internal/proc/process_linux.go)、[`findLongHexID / shortID`](../witr/internal/proc/container.go)、[`TestFindLongHexID / TestShortID`](../witr/internal/proc/container_test.go) | core 容器上下文（ContainerKey{runtime,id}） | 无 |
| 容器名称解析（inspect 把容器 ID 解析为可读名称，失败回退 "<runtime> (<短ID>)"） | parity | [`resolveContainerName`](../witr/internal/proc/container.go) | platform 容器运行时 | docker 用 compose 标签模板；podman/nerdctl 走 [`commandAsOriginalUser`](../witr/internal/proc/sudo_user_unix.go) 降权 |
| Docker/Podman/nerdctl 共享列表实现（ps --no-trunc --format，11 段竖线分隔模板） | parity | [`dockerLikeList（runtimeCommand + "ps" --no-trunc --format）`](../witr/internal/proc/runtime_dockerlike.go) | platform 容器运行时 | 无 |
| crictl 运行时（crictl ps -o json） | parity | [`crictlRuntime.List / crictlInspect`](../witr/internal/proc/runtime_crictl.go) | platform 容器运行时 | 无 |
| LXD/Incus 共享同一 REST JSON 结构 | parity | [`lxdLikeList / lxdLikeHostPID / lxdLikeEnrich`](../witr/internal/proc/runtime_lxdlike.go)、[`incusRuntime 复用 lxdLike*`](../witr/internal/proc/runtime_incus.go) | platform 容器运行时 | 无 |
| 容器运行时范围明确「FreeBSD jail 不实现」 | out of scope | [`jailRuntime`](../witr/internal/proc/runtime_jail_freebsd.go)（witr 已实现，Runquiry 明确排除） | — | FreeBSD 不在本期范围 |
| 容器标签页列表列（ID/Name/Runtime/Image/Status/Ports/Command） | parity | [`containerColumns`](../witr/internal/tui/model.go) | ui Containers 工作区 | 无 |
| 容器主机 PID 与富化 | parity | [`ResolveContainerHostPID`](../witr/internal/proc/container_runtime.go)、[`dockerLikeHostPID / dockerLikeEnrich`](../witr/internal/proc/runtime_dockerlike.go)、[`lxdLikeHostPID / lxdLikeEnrich`](../witr/internal/proc/runtime_lxdlike.go)、[`crictlRuntime.HostPID / crictlRuntime.Enrich`](../witr/internal/proc/runtime_crictl.go) | core + platform | 无 |
| 容器富化（Compose 信息除外） | out of scope | [`dockerLikeEnrich 的 Compose 标签读取`](../witr/internal/proc/runtime_dockerlike.go) | core 容器上下文（总计划未列） | 无 |
| Compose 项目/服务/配置文件/工作目录 | out of scope | [`ComposeProject / ComposeService / ComposeConfigFile / ComposeWorkingDir`](../witr/pkg/model/container.go) | 同 §1 Compose 条 | 无 |
| rootless 容器状态以当前用户可见 | parity | [`commandAsOriginalUser（sudo 下重建环境变量）`](../witr/internal/proc/sudo_user_unix.go) | platform CommandRunner | Windows 为 no-op（[`sudo_user_windows.go`](../witr/internal/proc/sudo_user_windows.go)） |
| 按端口发起的调查入口 | parity | [`ResolveContainerByPort`](../witr/internal/proc/container.go)、[`handleResolveError（ErrSocketOwnerUnknown 分支）`](../witr/internal/app/app.go) | core 端口调查 | 同 §2 对应条 |
| 容器运行时失败只生成 DiagnosticIssue | intentional change | witr 中不可用的运行时被静默跳过、错误不外抛：[`binAvailable`](../witr/internal/proc/runtime_dockerlike.go) 与 [`ResolveContainer / ListAllContainers 只遍历 Available() 的运行时`](../witr/internal/proc/container_runtime.go)；Runquiry 改为把失败记录为 DiagnosticIssue | platform CommandRunner | 无 |
| 容器按 runtime + id 去重 | parity | [`ResolveContainer / ListAllContainers（seen key = rt.Name()+"|"+c.ID）`](../witr/internal/proc/container_runtime.go) | core 外部命令边界 | 无 |

## 8. 刷新策略与并发

| 行为或能力 | 状态 | witr 证据 | Runquiry 对应模块 | 平台差异或说明 |
|---|---|---|---|---|
| 初始/基准刷新间隔为 3 秒（对齐 top 默认节奏） | parity | [`refreshInterval = 3 * time.Second`](../witr/internal/tui/constants.go) | core 刷新状态机 | 无 |
| 自适应刷新（连续两次超 60% 退避，低于 30% 加速，30%-60% 区间保持） | parity | [`maxRefreshInterval / refreshStep / slowFraction / fastFraction`（注释解释退避与加速）](../witr/internal/tui/constants.go) 与 [`adjustRefreshInterval`](../witr/internal/tui/update.go) | core 刷新状态机 | 无 |
| 同一工作区禁止重入刷新 | parity | [`refreshDue（refreshStartedAt 未超 maxRefreshInterval 时不再发起新刷新）`](../witr/internal/tui/update.go)、[`MainModel.refreshStartedAt` 注释（two don't overlap）](../witr/internal/tui/model.go) | core 刷新状态机 + ui B4 | 无 |
| 每个请求携带 generation，旧 generation 结果必须被丢弃 | out of scope | 无 witr 对应行为（`generation` 在 witr 源码中零命中；witr 用 [`refreshDue` 的 in-flight 门控](../witr/internal/tui/update.go) 与 [`pidIdentityChanged`](../witr/internal/tui/update.go) 兜底过期结果） | core 刷新状态机（Runquiry 设计项，非 witr 行为，不标 parity） | 无 |
| 手工刷新不得与正在执行的自动刷新并行 | parity | [`refreshDue`（in-flight 门控同样约束手动路径）](../witr/internal/tui/update.go) | core 刷新状态机 | 无 |
| 各工作区缓存 TTL（Linux 缓存 socket 表、macOS lsof/ps 结果、Windows 快照与服务映射） | parity | Linux [`socketCacheTTL`](../witr/internal/proc/net_linux.go)、macOS [`openPortsCacheTTL`](../witr/internal/proc/fd_darwin.go)、Windows [`snapshotCacheTTL`](../witr/internal/proc/snapshot_windows.go) 与 [`serviceMapCacheTTL`](../witr/internal/proc/services_windows.go) | platform 采集 | 各平台缓存对象不同 |

## 9. 进程操作

| 行为或能力 | 状态 | witr 证据 | Runquiry 对应模块 | 平台差异或说明 |
|---|---|---|---|---|
| 进程操作采用两步流程（操作菜单 + 确认步骤；Runquiry 将确认呈现为 AlertDialog） | intentional change | witr 已有两步流程：[`actionMenuSelect`（k/t/p/r/n/esc）与 `confirmKey`（y/n/esc）](../witr/internal/tui/action_fsm.go)、[update.go 的确认分支（`confirmExecute`/`confirmCancel`）](../witr/internal/tui/update.go)；Runquiry 仅把终端提示改为 GPUI `AlertDialog` | ui 安全交互 / app B7 | 非 Windows（[`actionsSupported` 门控](../witr/internal/tui/update.go)） |
| 进程操作前重读 ProcessIdentity | parity | [`pidIdentityChanged（比较 PID + StartedAt）`](../witr/internal/tui/update.go) | core + platform | 无 |
| 进程操作实现为信号（SIGKILL/SIGTERM/SIGSTOP/SIGCONT）与 setpriority(PRIO_PROCESS) | parity | [`killProcess/termProcess/pauseProcess/resumeProcess` 与 `sendSignal`](../witr/internal/tui/actions.go)、[`setNice 内的 syscall.Setpriority(PRIO_PROCESS, pid, value)`](../witr/internal/tui/actions.go) | platform ProcessController | Linux 同 witr；Windows 关闭类改用 TerminateProcess（intentional change，2026-09-12 用户裁决：仅关闭类；暂停/恢复/renice Unsupported，见 §10） |
| Runquiry 扩展：关闭进程树（KillTree）——目标先死、后代按快照 PPID 树逐个强杀，后代以快照 start_time 做 PID 复用防护 | intentional change（无 witr 参照；Runquiry 2026-09-12 新增） | 无（witr 仅单进程 kill/term，见 [`actions.go`](../witr/internal/tui/actions.go)） | core ProcessAction::KillTree + platform ProcessController + ui 右键菜单/动作面板 | Linux pidfd 逐进程 SIGKILL；Windows TerminateProcess；后代已退出按成功，其余失败聚合首错 |
| 进程操作入口增加行右键菜单（关闭进程/关闭进程树） | intentional change（无 witr 参照；witr 为键盘动作菜单） | witr 动作入口见 [`actionMenuSelect`](../witr/internal/tui/action_fsm.go) | ui Processes 表格行右键菜单 | 逐动作能力门禁：`ProcessController::action_capability`（Runquiry 扩展）不可用的动作不渲染入口 |
| Renice 仅允许 -20..=19，越界在调 syscall 前拒绝 | parity | [`setNice（value < -20 \|\| value > 19 先拒绝）`](../witr/internal/tui/actions.go)、[`validateNiceValue（输入解析期同样校验）`](../witr/internal/tui/action_fsm.go) | core ProcessAction（Rust 侧用 i8 承载该范围） | 仅非 Windows |
| 动作菜单快捷键（a 打开菜单，k/t/p/r/n 选动作，esc/q 取消） | parity | [`"a", "A" 打开菜单`](../witr/internal/tui/update.go)、[`actionMenuSelect 的 "k"/"t"/"p"/"r"/"n"/"esc"/"q"` 分支](../witr/internal/tui/action_fsm.go) | ui | Windows 不可达（actionsSupported 门控） |
| 确认文案展示动作名与 PID | parity | [`view.go 确认提示 "Kill PID %d? [y]es / [n]o" 等四种`](../witr/internal/tui/view.go) | ui 安全交互 | 无 |
| 确认后 kill/term 返回列表并触发刷新；pause/resume 留在详情页 | parity | [`update.go 的 originalAction switch（kill/term → stateList + refreshProcesses；default 留在详情）`](../witr/internal/tui/update.go) | ui B7 | 无 |
| PID 身份变化时早退并提示重试 | parity | [`pidIdentityChanged 命中时置 statusMsg "PID %d changed since opened — refresh and retry" 并早退`](../witr/internal/tui/update.go) | core/platform | 无 |
| 操作后错误状态显示 | intentional change | witr 以单条状态行呈现：[`update.go 的 m.statusMsg = fmt.Sprintf("Error: %v", execErr)`](../witr/internal/tui/update.go)；Runquiry 改为结构化错误状态矩阵 | ui 状态矩阵 | 无 |

## 10. Linux/Windows 平台差异

| 平台 | 能力差异 | 状态 | witr 证据 |
|---|---|---|---|
| Linux | /proc/net、/proc/locks、/proc/PID/io、cgroup、capabilities、systemd D-Bus、logind | parity | [`ReadProcess（/proc 采集）`](../witr/internal/proc/process_linux.go)、[`detectSystemd / enrichFromSystemd`](../witr/internal/source/systemd_linux.go)、[`resource_linux.go 的 checkPreventsSleep（logind D-Bus）`](../witr/internal/proc/resource_linux.go) |
| macOS | lsof -F（socket/文件锁）、libproc（能耗/热状态/App Nap）、pmset assertions、launchd/plist | out of scope | [`ResolveFile（lsof -F p）`](../witr/internal/target/file_darwin.go)、[`GetResourceContext / checkPreventsSleep / getThermalState`](../witr/internal/proc/resource_darwin.go)、[`detectLaunchd`](../witr/internal/source/launchd_darwin.go)；macOS 支持已移出 v1 范围 |
| Windows | ToolHelp32 快照、IP Helper API、PSAPI、Windows SCM；文件锁 Unsupported；进程操作仅关闭类（terminate/kill/kill-tree，Runquiry 扩展；暂停/恢复/renice Unsupported） | parity（文件锁；关闭类为 intentional change——witr 在 Windows 同样不支持，Runquiry 自 v1 起实现关闭类） | [`readEnvironmentBlock / parseEnvBlock（PEB）`](../witr/internal/proc/peb_windows.go)、[`detectWindowsService（三级判定）`](../witr/internal/source/service_windows.go)、[`serviceMapCacheTTL 服务映射缓存`](../witr/internal/proc/services_windows.go)；文件锁 Unsupported 见 [`locks_windows.go`](../witr/internal/proc/locks_windows.go)、witr 进程操作 Unsupported 见 [`actions_windows.go`](../witr/internal/tui/actions_windows.go)（Runquiry 与此不同，见 §9 进程操作与 2026-09-12 用户裁决） |
| 缓存 TTL | Linux socket 表缓存、macOS lsof/ps 缓存、Windows 快照/服务映射缓存 | parity | [`socketCacheTTL`](../witr/internal/proc/net_linux.go)、[`openPortsCacheTTL`](../witr/internal/proc/fd_darwin.go)、[`snapshotCacheTTL`](../witr/internal/proc/snapshot_windows.go)、[`serviceMapCacheTTL`](../witr/internal/proc/services_windows.go)（同 §8 缓存 TTL 条） |
| FreeBSD | 不在本期范围（supervisor/rc.d 名单仅作参考） | out of scope | witr 存在实现（[`name_freebsd.go`](../witr/internal/target/name_freebsd.go)、[`file_freebsd.go`](../witr/internal/target/file_freebsd.go)、[`runtime_jail_freebsd.go`](../witr/internal/proc/runtime_jail_freebsd.go)），Runquiry 平台矩阵明确排除 |

## 11. 其他行为（CLI 入口与输出格式）

| 行为或能力 | 状态 | witr 证据 |
|---|---|---|
| CLI 入口 main() 注入构建元信息后调用 app.Execute() | out of scope | [`main`](../witr/cmd/witr/main.go) |
| 空目标校验、多目标处理、--json 顶层数组、退出码契约 | out of scope | [`runApp（len(targets)==0 / jsonResults / highestExit）`](../witr/internal/app/app.go)、[`processTarget`](../witr/internal/app/app.go)（语义细节见 §3 对应条） |
| JSON 输出变体（standard/short/tree/warnings 等 To*JSON） | out of scope | [`ToJSON / ToShortJSON / ToTreeJSON / ToWarningsJSON`](../witr/internal/output/json.go) |
| 终端输出净化（控制字符与非法 UTF-8 替换） | out of scope | [`SanitizeTerminal / SanitizeTerminalLine`](../witr/internal/output/sanitize.go) |
| 颜色决策（--no-color / NO_COLOR / 非 TTY 禁色） | out of scope | [`useColor`（flags.noColor、NO_COLOR、isTerminal）](../witr/internal/app/color.go) |
| 启动时间展示格式 | out of scope | [`FormatStartedAt`](../witr/internal/output/started.go)、[`humanDuration / formatRelativeTime`](../witr/internal/source/systemd_linux.go)（同 §1 时间格式化条） |
| exitCodeError 包装 | out of scope | [`exitCodeError`](../witr/internal/app/app.go)、[`ExitOK..ExitInternalError 常量`](../witr/internal/app/app.go)（同 §3 退出码契约条） |

## 12. 四个工作区与 witr TUI 标签页对照

Runquiry 的四个工作区一一对应 witr TUI 的四个标签页（[`tabProcesses / tabPorts / tabContainers / tabLocks`](../witr/internal/tui/model.go)，`type tab int` 枚举；标签页切换见 [`view.go`](../witr/internal/tui/view.go)）：

| Runquiry 工作区 | witr TUI 标签页 | witr 证据 |
|---|---|---|
| Processes | `tabProcesses`（进程表 + 详情侧栏） | [`MainModel（containerTable/portTable/lockTable 并列，activeTab 选择）`](../witr/internal/tui/model.go) |
| Ports | `tabPorts`（端口表 + `showAllPorts` 切换） | [`portTable / portDetailTable`](../witr/internal/tui/model.go)、[`"a", "A" 的 tabPorts 分支`](../witr/internal/tui/update.go) |
| Containers | `tabContainers`（容器表，`containerColumns`） | [`containerColumns`](../witr/internal/tui/model.go)、[`ListAllContainers（供 TUI 容器标签页）`](../witr/internal/proc/container_runtime.go) |
| File Locks | `tabLocks`（锁表，Windows 降级） | [`lockTable`](../witr/internal/tui/model.go)、[`locks_supported.go`](../witr/internal/tui/locks_supported.go) 与 [`locks_windows.go`](../witr/internal/tui/locks_windows.go) |

各工作区的刷新、多结果候选表、Unsupported 展示等行为分别见 §2、§7、§8 与 §10；本节仅锚定「四工作区 ↔ witr 四标签页」的结构对应。

## 验收核对

- 行为条目总数：202 条（§1 领域模型 35、§2 目标 40、§3 解析补充 20、§4 管线 27、§5 来源 21、§6 告警 12、§7 容器运行时 16、§8 刷新 6、§9 进程操作 9、§10 平台差异 5、§11 其他 7、§12 四工作区 4）。
- 无来源条目数：0（每条的证据列均含指向 `../witr/` 下具体文件与符号的链接；仅 §8 generation 条与 §10 FreeBSD 行标注为「无 witr 对应行为 / witr 已实现但明确排除」，并各自给出佐证链接）。
- 跨章节重复条目已标注「同 §X 对应条」，同章节内的逐字重复行已删除。
- 覆盖主题：四个工作区（Processes/Ports/Containers/File Locks，见 §12）、五类调查目标（名称/PID/端口/文件/容器）、目标解析与多结果行为、来源识别及优先级、告警规则、容器运行时、刷新策略、进程操作、Linux/Windows 平台差异、领域模型、其他行为（CLI 入口与输出格式）。
- macOS 条目处置（2026-09-08）：纯 macOS 专属条目改标 `out of scope`；跨平台条目保留 `parity`，其备注中的 macOS 实现方式仅作 witr 行为记录。
