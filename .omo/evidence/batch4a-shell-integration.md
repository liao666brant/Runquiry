# Batch 4A AppShell 集成证据

日期：2026-09-04

## DoneClaim

PASS。Runquiry 产品窗口已由 app 装配真实 Linux 平台后端，四个工作区、五类显式调查入口、稳定选择、generation 门控、宽/窄窗实时详情和双语文案均已接入。I1 复审提出的五项阻断已关闭：Sheet 绑定可更新 Entity；容器无 verified host PID 保留容器结果；三清单详情按稳定键读取当前快照；真实交互统一推进 AppSession generation；生产 KeyBinding 消费 ProcessCommand。B7 进程控制未实现且界面明确禁用；未提交、未推送、未改依赖。

## 红绿记录

- baseline：`cargo test -p runquiry-ui --locked` 为 42/42；`cargo test -p runquiry-app --locked` 为 14/14。
- red：先加入后端边界门控测试；`cargo test -p runquiry-ui --locked` 首次因 `WorkspaceResultGate` 尚未定义而以 E0432 失败，证明测试先于实现。
- I1 red：先加入稳定键详情与真实会话交互回归，UI 首轮出现 8 个预期的缺失 API 编译错误；实现后最终 UI 47/47、app 15/15。
- I1 green：`fake_backend_result_is_rejected_after_real_session_interactions` 覆盖 filter/selection/mode/sort 对在途快照的拒绝；app 生产容器转换测试证明唯一容器无 verified host PID 返回容器 fallback 而非 NotFound。
- 视觉 red/green：初次真实窗口截图发现主区未纵向撑满，真实 `DataTable` 高度为零（`/tmp/runquiry-batch4a-processes-after.png`）；给主区和详情区补齐 `h_full` 后，真实进程行、列头和滚动区可见（`/tmp/runquiry-batch4a-processes-fixed.png`）。

## 改动清单

- `runquiry-ui::backend`：最小 `WorkspaceBackend`、不可变 `WorkspaceSnapshot`、`WorkspaceResultGate`。
- `runquiry-app::backend`：共享 `Arc<LinuxPlatform>` 与 `Arc<ContainerRuntimes>`，装配 process/network/container/file inventory、core resolve/analyze；无假数据。
- `AppShell`：后台 executor 采集与分析、workspace/generation/identity 门控、任务生命周期、自动/手动同路刷新。
- 四工作区：真实 `TableState`/`DataTable`、领域稳定 ID 选择、文本筛选、排序/模式切换、partial 数据与具体诊断。
- 调查栏：name/PID/port/file/container 显式类型、core parse/resolve、zero/invalid/error、unique/ambiguous、候选选择与 500ms 后台详情加载；零命中有独立 Empty 呈现。
- 详情：Process 概览、祖先、来源、告警、资源、Socket、文件锁、环境的真实明细；partial 具体问题；默认脱敏、当前详情 reveal；B7 操作明确禁用。
- 快捷键：Ctrl/Cmd+K、Ctrl/Cmd+R 与 Ctrl/Cmd+1..4 均绑定同一 shell action。
- 响应式：宽窗 65/35；960–1099px 窄窗使用观察 AppShell 的 `Root` Sheet，异步分析与新快照自行重绘；Escape 关闭后 DataTable 继续接收方向键。
- i18n：新增产品文案及 B6 表头/占位 en、zh-CN；运行时诊断原文不伪翻译。

## 用户可达矩阵

| 路径 | 真实观察 | 证据 |
|---|---|---|
| Processes | 真实本机进程 DataTable 可见；960px 同一 PID 的 Sheet 从 PID 占位自行更新为完整 Analysis | `/tmp/runquiry-batch4a-i1-process-sheet-before.png`、`/tmp/runquiry-batch4a-i1-process-sheet-after.png` |
| Ports | 真实监听 socket；宽窗与 1099/960px Sheet 均显示 protocol/address/port/state/PID/process/public-bind 及具体 issue | `/tmp/runquiry-batch4a-i1-ports-detail-wide.png`、`/tmp/runquiry-batch4a-i1-ports-sheet-1099.png`、`/tmp/runquiry-batch4a-i1-ports-sheet-960.png` |
| Containers | `Ctrl+3` 的生产 binding 真实到达；本机 docker 可探测但清单失败时显示 Error 与 6 项具体能力/工具诊断。无 host PID fallback 由 production conversion contract 覆盖，不伪造本机容器 | `/tmp/runquiry-batch4a-i1-shortcut-ctrl3.png`、`/tmp/runquiry-batch4a-i1-containers-real-960.png`、app test log |
| File Locks | 真实锁行；宽/窄详情显示 path/PID/process/FD/type/mode 及 3 项具体 issue | `/tmp/runquiry-batch4a-i1-files-detail-wide.png`、`/tmp/runquiry-batch4a-i1-files-sheet-960.png` |
| 焦点恢复 | 960px 端口 Sheet 按 Escape 关闭后，Down 使选中从 `:::2222` 移至 `0.0.0.0:2222` 并重开对应 Sheet | `/tmp/runquiry-batch4a-i1-ports-sheet-960.png`、`/tmp/runquiry-batch4a-i1-ports-focus-restored-960.png` |

## 验证门

| 场景 | 调用 | 二进制可观察结果 | 原始产物 |
|---|---|---|---|
| core 回归 | `cargo test -p runquiry-core --locked` | 107 passed, 0 failed | `/tmp/runquiry-batch4a-final-gates.log` |
| platform 回归 | `cargo test -p runquiry-platform --locked` | 97 passed, 0 failed | `/tmp/runquiry-batch4a-final-gates.log` |
| UI 回归 | `cargo test -p runquiry-ui --locked` | 47 passed, 0 failed | `/tmp/runquiry-batch4a-i1-ui.log` |
| app 回归 | `cargo test -p runquiry-app --locked` | 15 passed, 0 failed | `/tmp/runquiry-batch4a-i1-app.log` |
| app 编译 | `cargo check -p runquiry-app --locked` | exit 0 | `/tmp/runquiry-batch4a-i1-check.log` |
| 严格 lint | `cargo clippy -p runquiry-core -p runquiry-platform -p runquiry-ui -p runquiry-app --all-targets --locked -- -D warnings -A clippy::multiple_crate_versions -A clippy::print_stderr` | exit 0；只使用任务批准的两个既定例外 | `/tmp/runquiry-batch4a-i1-clippy.log` |
| 格式与 diff | `cargo fmt --check`；`git diff --check` | 均 exit 0 | `/tmp/runquiry-batch4a-i1-fmt.log`、`/tmp/runquiry-batch4a-i1-diff.log` |
| 文件规模 | 既定 awk 纯 LOC 统计（测试模块不计生产） | 本轮生产文件最大 232，均 `<250` | `/tmp/runquiry-batch4a-i1-loc.log` |
| 真实启动 | `env DISPLAY=:0 WAYLAND_DISPLAY= target/debug/runquiry` | 当前构建 X11 window `6291457`；960、1099、1280 三档均真实渲染 | 上述 I1 PNG |
| 进程清理 | 启动会话发送 Ctrl-C；`pgrep -a -x runquiry` | 无输出 | 本报告记录；PTY session 61973 已关闭 |

## 未完成项

- B7 的真实进程控制按边界未实现；当前只显示禁用说明，没有假成功按钮。
- 本轮只验证 Linux 平台真实装配；macOS/Windows 平台实现不在 Batch 4A 范围。
- 100k 测试证明现有 `TableState`/`DataTable` seam 保持索引快照；真实窗口滚动证据仅覆盖本机实际行数，不声称 100k 真实窗口虚拟化已经视觉验收。
- 本机没有可成功列出的容器，故真实窗口只能验证诚实的 unavailable/error 状态；无 host PID 的容器详情由 production conversion contract 与 UI fallback 类型测试证明，不声称有本机容器截图。
