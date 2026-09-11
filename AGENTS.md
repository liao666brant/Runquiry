# Runquiry

对本地 [witr](witr/README.md)（Go CLI/TUI）的纯 Rust + GPUI 桌面化重写：原生桌面 GUI，提供 Processes、Ports、Containers、File Locks 四个工作区与按名称、PID、端口、文件、容器发起的调查。不嵌入 Go，不新增 CLI/TUI；运行期完全本地（无遥测、云服务、自动更新或后台网络请求）。目标平台 Linux 与 Windows，交付无签名安装包。

## 架构总览

Cargo workspace（resolver = "3"），四层单向依赖：

```mermaid
graph TD
    APP[runquiry-app<br/>装配与入口]
    UI[runquiry-ui<br/>GPUI 界面]
    PLAT[runquiry-platform<br/>平台采集与控制]
    CORE[runquiry-core<br/>领域模型与端口]
    APP --> UI
    APP --> PLAT
    APP --> CORE
    UI --> CORE
    PLAT --> CORE
```

- **runquiry-core**：领域模型、目标解析、分析管线、告警规则、刷新状态机、平台端口（trait）。不依赖 GPUI 或操作系统。
- **runquiry-platform**：core 端口的 Linux/Windows 双平台实现（采集器、容器运行时、进程控制、外部命令执行）。
- **runquiry-ui**：GPUI 状态、设计系统、四个工作区、调查面板、设置与国际化。
- **runquiry-app**：可执行装配层（窗口启动、配置持久化、资源与打包元数据）。

参考代码 [witr/](witr/README.md) 是行为契约的静态阅读来源，**不参与构建、不得修改、不入 Git**。

## 模块索引

| 模块 | 职责 | 文档 |
|---|---|---|
| crates/runquiry-core | 领域模型、目标解析、管线与平台端口 | [AGENTS.md](crates/runquiry-core/AGENTS.md) |
| crates/runquiry-platform | 平台采集器、容器运行时与进程控制 | [AGENTS.md](crates/runquiry-platform/AGENTS.md) |
| crates/runquiry-ui | GPUI 界面：工作区、调查面板与设计系统 | [AGENTS.md](crates/runquiry-ui/AGENTS.md) |
| crates/runquiry-app | 依赖装配、窗口启动与打包元数据 | [AGENTS.md](crates/runquiry-app/AGENTS.md) |
| docs/witr-parity.md | witr 行为契约（202 条，Runquiry 语义的唯一来源） | [witr-parity.md](docs/witr-parity.md) |
| .omo/plans/runquiry-gpui-desktop/ | 7 个模块的实施计划与任务勾选 | [总计划](.omo/plans/runquiry-gpui-desktop.md) |

## 运行与开发

```bash
cargo check -p runquiry-app --locked   # 编译检查（必须 --locked）
cargo run -p runquiry-app --locked     # 启动产品壳层与本地采集后端
cargo fmt --check                      # 格式检查
cargo deny check                       # 许可证/ advisories / 来源检查（需 cargo-deny）
```

- 工具链由 [rust-toolchain.toml](rust-toolchain.toml) 锁定 1.95.0（1.90–1.94 均实测编译失败，证据见模块 01 计划）。
- Linux 构建需系统依赖：pkg-config、libfontconfig1-dev、libfreetype-dev、libxkbcommon-dev、libxkbcommon-x11-dev、libwayland-dev、wayland-protocols、libx11-dev。
- WSLg 下 libEGL/MESA 软件渲染警告属正常。

## 依赖锁定（关键约束）

- GPUI Kit 在根 `Cargo.toml` 的 workspace dependencies 中精确锁定为 `=0.6.1`，runquiry-app 与 runquiry-ui 统一继承；GPUI、组件与资源分别由 `gpui_kit`、`gpui_kit::component`、`gpui_kit::assets` 提供。
- 配套 `gpui-pre-*` 由 `Cargo.lock` 锁定为 0.3.4，依赖来源统一为 crates.io；不得混入旧 GPUI/git 组件依赖。
- **禁止无差别 `cargo update`**；所有验证、CI、打包必须 `--locked`。
- 新增依赖由模块 01（A1）负责人集中修改并重新验证 GPUI 类型与来源一致性。

## 测试策略

- Batch 6 前基线为四 crate 273 个测试：`cargo test -p runquiry-core --locked`（107）、`-p runquiry-platform`（97）、`-p runquiry-ui`（48）、`-p runquiry-app`（21）；B7 独立结果为 platform `process_controller` 6/6、UI 完整套件 57/57，随后确认态定向测试 1/1，app 24/24；这些调用不可相加推断为本轮全量测试或“58 个 UI 全跑”。`cargo test -p runquiry-ui` 等禁止给 ui 加 gpui test-support dev-dependency（新增测试依赖须经依赖守门人核验），纯逻辑一律普通 `#[test]`。Batch 6 当前实测：core 110/110（+3 个 launchd/Windows service/init 来源判定测试）、app 24/24；platform 新增 `tests/windows_*` 纯解析套件（约 20+ 测试，经 `#[path]` 引入 src 纯模块；`tests/macos_*` 已随 macOS 移出 v1 范围删除）。Batch 7A C3 新增：UI `capability_contract_tests` 7 个平台风格假后端契约测试 + `from_parts` 状态推导断言测试、app `failed_collection_error` 映射测试，**全部已编写、未运行**（延续构建禁令，见 `.omo/evidence/batch7a-result.md`），运行后以实际数字更新各模块 AGENTS。
- 合成 fixture 与失败注入在 workspace 根 `tests/fixtures/`（清单与敏感信息扫描见其 README.md）；SocketEntry 输入边界规则（TCP/UDP 必有合法端口、Unix 必无端口）在 fixture loader 层执行，平台真实输入必须复用。
- 行为语义以 [docs/witr-parity.md](docs/witr-parity.md) 为验收依据；fixture 要求合成值（无真实用户名、路径、Token）。

## 编码规范

- Rust edition 2024；rustfmt 由 [rustfmt.toml](rustfmt.toml) 约定（max_width = 100）。
- lint 基线在根 `Cargo.toml` `[workspace.lints]`：clippy all = deny、pedantic/nursery/cargo = warn；`unwrap_used`/`expect_used`/`panic`/`todo`/`unimplemented` 均 deny；`missing_docs` = warn。
- 依赖方向单向：core 不依赖 GPUI/OS；platform 与 ui 只依赖 core；ui 禁止直接读 /proc、调用 Win32 API 或运行 lsof/容器 CLI。
- 许可证：项目 GPL-3.0-or-later（[LICENSE](LICENSE)）；witr 归属见 [NOTICE](NOTICE)；依赖许可证与来源限制以 `deny.toml` 为准，GPUI Kit 迁移后的 crates.io 依赖不再沿用旧 Zed git 包的 GPL 例外与 license clarify。

## AI 使用指引

- 任何功能实现前先读 [docs/witr-parity.md](docs/witr-parity.md) 对应章节——它逐项定义 parity / intentional change / out of scope，无需重新决定 witr 语义。
- 实施计划与任务勾选在 [.omo/plans/runquiry-gpui-desktop/](.omo/plans/runquiry-gpui-desktop/)；共享根文件（根 Cargo.toml、Cargo.lock、deny.toml、rust-toolchain.toml）只允许模块 01 负责人写入。
- 不修改 `witr/` 参考源码；不顺带升级计划外依赖。
- GPUI Kit API 与框架机制参考 `.agents/skills/gpui-kit/`；设计与交互变更先读 `.agents/skills/gpui-kit-design-guides/`。`gpui_kit::component::Root` 必须是每个窗口的第一级视图。

## 变更记录

- 2026-09-02 @9d4a616：A1 工程基线（四层 workspace、依赖锁定、最小窗口）、A2 witr 行为契约落地；初次建立 AI 上下文索引。
- 2026-09-03（未提交工作区）：Batch 1——A3 核心领域类型与七平台端口落地（serde 经守门人批准，零新增包）；A4 设计系统（DESIGN.md + runquiry-ui 主题/状态/双语占位）与独立 gallery 实验台（runquiry-app/examples/gallery）；gpui/gpui-component git 依赖同步内联至 runquiry-ui。
- 2026-09-03（未提交工作区）：Batch 2——Phase 0 修复 gallery Name 列省略/tooltip/Sheet 绑定行并复验（qa-report §10）；A5 三平台 fixture + 失败注入 + ProcessController 契约修正（tests/fixtures/）；B4 产品壳层（runquiry-app 装配 + runquiry-ui shell/session/debounce）+ core 纯刷新策略 + 设置持久化 allowlist + rust-i18n 迁移（runquiry-ui/locales，新增 rust-i18n/serde/serde_json 依赖均为 lock 内既有版本，零新增包）。
- 2026-09-03（未提交工作区）：Batch 2 评审修复——core 刷新完成/中止绑定 generation（过期信号不释放新请求）；UI 落实 1099/1100px 响应式断点并使用 Unsupported 能力边界态；app 以真实 bounds 事件持久化窗口尺寸，设置写入改为绝对安全路径、唯一排他临时文件与 Unix 私有权限；补齐 Gallery 第二行 → Sheet 及四工作区宽/窄窗视觉证据；测试增至 82 个。
- 2026-09-04 @984999c：Batch 3——B1 目标解析与分析管线；B2 Linux 只读适配器；B3 受限 CommandRunner 与七容器运行时；新增直接依赖 sysinfo/serde/serde_json/zbus/libc（全部锁内既有包，GPUI source 未漂移）。首轮评审已修复 analyze 单快照、祖先链诊断、健康标签、收养后代排除、ProcessFileLocks 与 ContainerHealthcheckProbe。当时未做 B5/B6/B7/B8、macOS/Windows、打包；B5/B6 已在下条 Batch 4A 落地。
- 2026-09-04（未提交工作区）：Batch 3 阻断项修复——命令输出超限/取消/读取失败均类型化失败并回收进程组，修复双流竞态；sudo 下 Podman/nerdctl 恢复原用户；容器 command/Compose 五字段仅留私有临时结构，nerdctl 稳定键改为 containerd，补 Docker 发布端口回退；host PID 经 Linux cgroup 二次验证；生产 PID 枚举回归 sysinfo，详情字段失败保留部分结果，构造墙钟生效，systemd D-Bus 加方法超时与 single-flight；超长 core/platform 测试拆分。测试增至 232 个，本轮改动的 core/platform 文件均低于 250 纯代码行。
- 2026-09-04（未提交工作区）：Batch 4A——P1 将 `FileInventory` 扩展为可见打开文件与真实锁的列表/路径持有者契约，Linux 以 `/proc/PID/fd` 与 `/proc/locks` 采集并保留有界诊断；B5 完成进程列表、筛选/排序、五类查询、分析详情与操作入口的安全禁用态；B6 完成 Ports、Containers、File Locks 的真实快照、表格与详情。I1 在 app 边界装配本地平台后端：每次 load/resolve/analyze 新建 `LinuxPlatform`，共享 `AnalysisGate` 保留跨请求分析互斥；宽窗使用真实 `h_resizable` 详情分栏，窄窗改用 Sheet；语言切换同步既有 `InputState` 占位符。测试基线为 core 107、platform 97、ui 48、app 21。
- 2026-09-07（未提交工作区）：Batch 4B B7——Linux 进程控制以 pidfd 绑定 TERM/KILL/STOP/CONT，renice 明确保留 PID 复用 TOCTOU；UI/App 接入能力态、二次确认、键盘与异步刷新桥接。真实 X11 QA 已完成五类动作、非法输入、权限边界、输入焦点及 Sheet/Dialog 分层与 Escape 验收；B8 尚未开始，不据此宣称全平台验收。

- 2026-09-07 @0c476b5（Batch 5）：B8 Linux X11/Wayland 真实验收通过，独立门禁 CONFIRMED（见 .omo/evidence/batch5-*），C1/C2 前置门解除。
- 2026-09-07（未提交工作区）：Batch 6 C1/C2——macOS（`src/macos/`，libproc 手写绑定 + lsof -F + launchctl/plist + kill(2)/setpriority 控制）与 Windows（`src/windows/`，windows-sys 0.61.2 安全包装 + IP Helper + PEB/PEB32 有界读取 + SCM 证据，File Locks 与进程控制 Unsupported）适配器代码落盘；core `SourceEvidence` 加性扩展 launchd/Windows service 证据并补齐来源判定链（core 110 测试全绿）；app backend 按 target_os 装配平台别名（Linux 24/24 回归通过）；依赖守门新增 windows-sys 0.61.2（锁内既有版本，Cargo.lock 仅 +1 行依赖边，GPUI source 未漂移）。**所有 macOS/Windows cfg 代码未编译、未测试**（WSL 编译链接两次卡死，用户叫停后续构建），C1/C2 保持未完成状态，实机验收未开始；独立 FFI 审查发现并修复 2 处阻断 + 3 处建议缺陷；随后静态 code-review（双轴）修复 app `ContainerRuntimes` cfg 导入错误、25 处测试 unwrap 基线违规、IP Helper 重试丢尺寸、Windows start_time 0 语义，并接线 parity line 88 的 macOS launchd 名称解析回退。

- 2026-09-07（未提交工作区）：Batch 7A C3（能力矩阵与条件 UI，验证阻断）——`DataState` 新增 `Unavailable` 环境边界态（DESIGN §6 六种呈现状态、图标 `info`，gallery CLI/文案同步）；产品工作区状态文案统一走 `locale::workspace_state_copy`，删除死键 `main.collector_unavailable.*`；`LoadPresentation::boundary_reason` 保留 Unsupported/Unavailable 平台原因并在 StateView 透出（Windows File Locks 保留导航入口并解释原因），能力边界下模式/筛选/排序按钮禁用；进程控制能力随 Processes 刷新在后台动态取回，能力退化经 `ProcessActionFlow::revoke_confirmation_if_unusable` 撤销确认、提交前再门禁；动作错误补 `actions.error.unsupported`/`actions.error.external_tool` 键。app 边界：`process_control_capability` 平台构造失败改 `CapabilityStatus::Unavailable`，resolve 端口/文件采集失败经 `failed_collection_error` 保留平台诊断。新增 UI 7 个三平台假后端契约测试（复用生产状态转换，纯 `#[test]`）与 app 1 个映射测试，**全部已编写、未运行**（延续构建禁令），C3 保持未勾选，不解除 C1/C2 前置验收要求；不改 core/platform/Cargo.lock/依赖。code-review（双轴）修复：Processes 页失败采集不再伪装成空集合（新增 `SurfaceState::from_parts` 与清单 `map_state` 同语义，`state_from_inspection` 改部件签名为单一事实源），Processes 交互门控与边界判定同源；平台原因本地化维持「UI 双语 + 原因原样附显」折中（修复属 core/platform 范围）。
- 2026-09-07（未提交工作区）：Batch 7B C4（套件准备，验证阻断）——三平台契约覆盖矩阵、静态差异复核与缺口清单落地（`.omo/evidence/batch7b-c4-matrix.md`，静态盘点 core 111 / platform 188 / ui 69 / app 28 个测试（ui 含本轮 2 个新门禁测试），最后全绿基线 core 110、platform 97、ui 57、app 24）；提交前再门禁提取为 `ProcessActionFlow::confirm_if_usable` 并补 2 个门禁测试（闭合 H 类确认门禁断言缺口）。差异分类：Unavailable 跨路径表达不一致为已记录低危实现缺陷（根因 core 无 Unavailable 错误变体）、平台原因不本地化为计划明示折中（全部中文硬编码产生点已定位）、能力退化撤销与提交门禁传播完整无绕过。环境盘点：无 macOS 主机；WSL 后发现 Windows 主机（rustup 1.98.0-msvc）待授权；交叉目标装在 1.98.0 与锁定 1.95.0 不一致。C1/C2 编译与实机证据仍为零，C4 保持未完成、v1 契约未冻结、不开始 D1/D2；本轮零依赖改动、零 Cargo.lock 改动。
- 2026-09-07（未提交工作区）：Windows 主机验证通道打通——环境确认 rustup 1.95.0-msvc + VS Build Tools 17.14 可编译（cargo-deny 未安装），修复 ui/app 层 Batch 7A 遗留编译错误后工作区全量编译通过；platform 删除 8 项 dead-code（winerror 常量与 `details_error_for`、ip_table `to_open_port`/`UDP_STATE`、process_list `snapshot_entry`、peb_reader `is_empty`、scm_parse `start_mode_name`、toolhelp `threads` 字段）并清零编译警告（`#[path]` 测试目标加针对性 allow、`cfg(unix)` 移至文档后、重导出按消费方拆分）；clippy 首次在 Windows/macOS 代码上运行，修复 51 处 deny 级错误——根因一：FFI 审查所加 `// SAFETY：` 用全角冒号未被 `undocumented_unsafe_blocks` 识别（统一改半角即消除 24 处误报），根因二：真实缺陷（9 处内联 `Win32Error(unsafe { GetLastError() })` 拆独立语句、3 处 `field_reassign_with_default` 改结构体初始化、TCP/UDP 复合 unsafe 块拆分满足 `multiple_unsafe_ops_per_block`、macOS 解析模块嵌套 if 折叠 let 链、恒真运行期断言改 `const` 断言、`ffi_scm` 冗余 `#[must_use]` 与手写切片字节数）；platform 全部测试通过（lib 48、fake_backends 10、macos_identity 6、macos_lsof 11、windows_* 全绿）；`windows_qa` 只读实机冒烟通过（252 进程 / 159 端口 / SCM 3 PID 证据，File Locks 与进程控制正确报 Unsupported）——C2 首份实机证据。macos_launchd_parse 在 Windows 验证主机上不稳定（0xc0000409 秒崩与 plist 测试挂起漂移，独立最小复现亦崩），疑似内存硬件问题（用户侧待 mdsched/MemTest86）；**用户确认无 macOS 设备，该套件验证搁置、仅作记录，不阻塞其余工作**。
- 2026-09-07（未提交工作区）：Batch 7 自动验证闭合与 C2 实机证据扩展——`cargo test --workspace --locked --no-fail-fast` 在 Windows 全绿（core 全绿、platform 除搁置的 macos_launchd_parse 外全绿、ui 69/69 含 Batch 7A 的 7 个三平台契约测试与 2 个确认门禁测试、app 全绿），**C3 自动验证缺口闭合**（勾选仍待真实 GUI 交互/视觉矩阵验收）；windows_qa 扩展三个实机验收场景：受保护进程（csrss/winlogon）详情 Ok + `PermissionDenied` 诊断 + sysinfo 基线保留（部分结果契约）、32 位进程 PEB32 详情（环境块 55 项、工作目录可得）、容器 CLI 三者均未安装（「未安装」场景，另两场景该环境不适用）；新增 `process_list::snapshot_live` ToolHelp32 真机测试（lib 49 全绿）；GUI 启动冒烟通过（runquiry.exe 窗口 Responding，GPUI Windows 后端可用）；07 模块计划 C2 进度补记（签名确认、ToolHelp 回退真机验证闭合），C2 剩 GUI 交互矩阵、容器未启动/已启动场景（该环境无 Docker）、SCM QA 采样说明。
- 2026-09-07（未提交工作区）：cargo-deny 安装并接入验证（用户授权全局安装，`cargo install cargo-deny --locked`）；首次 `cargo deny check` licenses 失败——zed 固定提交内 `gpui_shared_string`/`gpui_util` manifest 未声明 license 字段，`deny.toml` 补两个 `[[licenses.clarify]]`（Apache-2.0，指纹 0x8a8d02f6）后**四项全绿**（advisories/bans/licenses/sources ok）。**用户决定：Linux/WSL 侧验证不在当前 Windows 验证环境进行，后续 Linux 验证需另行环境**（Linux 侧历史证据见 Batch 3–5 与 .omo/evidence）。
- 2026-09-07（未提交工作区）：Windows 标题栏与操作栏合并（用户请求）——app `WindowOptions` 改用 `TitleBar::window_options()`（原生标题栏透明 + `app_owns_titlebar_drag`），UI `render_toolbar` 改为 `render_title_bar`：左侧窗口名与刷新/主题/语言按钮同置（组间距 16px），窗口控制按钮由 `TitleBar` 经 `WindowControlArea` 自绘交给系统；`toolbar_focus` 与 tab 顺序保持不变。侧栏宽度经 224→176→100 多轮调整最终定为 120px（用户确认），主区初始宽度计算同步。**GUI 视觉与交互用户确认通过**。`DESIGN.md` §5/§7/§8 与 ui/app 模块 `AGENTS.md` 已同步；顺带修复 ui 首次过 clippy 暴露的 `surface.rs` deny 级 `redundant_guards`（改 `Some([])` 切片模式）；ui 69/69、ui/app clippy 零 error、fmt 干净。
- 2026-09-08（未提交工作区）：macOS 支持移出 v1 范围（用户决策，v1 收窄为 Linux 与 Windows）——删除 `crates/runquiry-platform/src/macos/`（libproc/lsof/launchctl/plist，15 文件 2660 行）、`examples/macos_qa.rs`、`tests/macos_{identity,launchd,lsof}_parse.rs` 与 `tests/fixtures/macos/`（10 个 fixture）；app 删除 `MacosPlatform` 平台别名、`launchd_service_pid` 名称解析回退（parity line 88）与 settings 的 macOS 配置路径分支；core fixture 测试平台矩阵收窄为 linux/windows；UI `MacosStyleBackend` 改名 `PartialFilesBackend`（该场景平台无关）；platform/app 增加 `compile_error!` 平台门禁。core 保留 `SourceType::Launchd`/`launchd_by_pid`/`detect_launchd` 领域类型与判定链（平台无关层）。`docs/witr-parity.md` 的 macOS 专属条目改标 out of scope。验证：四 crate `cargo check --all-targets` 全绿；core 109 + platform 180（合计 289）测试全绿、app 24 全绿；ui 68/69，唯一失败为既有测试过期（`render.rs` 仍按 224px 侧栏计算可用宽度），非本次引入。
- 2026-09-10：C4 Linux 侧契约回归（WSL2，`env -u RUSTUP_TOOLCHAIN` 使 `rust-toolchain.toml` 的 1.95.0 生效；基线 `5a837d0`）——`cargo fmt --check` 干净，core 109、platform 180（含 `windows_*` 纯解析 77）、ui 69、app 24 全绿，`cargo test --workspace --all-targets --locked` 385/385 退出码 0，clippy 零 error，`check --examples` 通过（`.omo/evidence/c4-linux-verification.md`，日志同目录）。修复两处既有缺口：侧栏宽度提取 `runquiry-ui::shell::render::SIDEBAR_WIDTH` 常量（宽度计算、渲染、测试断言共用，修复 `wide_layout_starts_with_a_65_35_split_after_the_sidebar` 按 224px 计算的过期断言）、删除 `processes/tests.rs` 未使用的 `supported` 变量。**C4 仍未完成**：Windows 侧需在 `5a837d0` 之后重跑全量（其全绿记录早于 macOS 移除提交）；差异 #1/#2（core 缺 Unavailable 错误变体、平台原因无稳定原因码）待裁决，#3/#4 待实机复核；C3 的 Linux GUI 交互回归未做。零依赖改动、未触碰 Cargo.lock。
- 2026-09-11（未提交工作区）：C 阶段推进（Windows 通道）——① ETXTBSY flaky 根因分析：Rust 探针实测 WSL2 内核「写后立即 execve」瞬态 ETXTBSY（plain ~3–6%、fsync 无效、2ms sleep 0/2400；愈合空载 ≤ ~0.9ms、高负载 ≤ ~5.7ms，远小于生产 spawn ~75ms 有界重试窗口），排除「残留执行中脚本写入碰撞」假说（shell 脚本非内核执行 inode）；`container_support.rs` 注释按实测数据更新（零行为差异），证据与 C4 判定口径落盘 `.omo/evidence/etxtbsy-flaky-analysis.md`；② Windows 侧全量回归闭合：`cargo test --workspace --all-targets --locked --no-fail-fast` 35 目标 343/343 全绿（差集 51 个为 cfg(unix) 门控套件），两平台同基线要求满足；通道环境约束 `CARGO_INCREMENTAL=0`（9p 增量锁文件创建失败）；③ `windows_qa` 基线后重跑通过（进程 421/端口 693/SCM 1 PID/受保护进程部分结果/PEB32 59 项/容器 CLI 未安装），证据 `.omo/evidence/c4-windows-{verification,all-targets.log,qa-post-baseline.log}`；C4 计划记录同步。C2/C3 的 Windows GUI 走查部分完成后**自动化中止**：SendKeys 按焦点投递在竞争时误将输入送入用户其他窗口（含文件管理器地址栏，替用户打开非预期窗口），经用户指出后立即停止全部键鼠自动化并清理（关闭所启 runquiry 实例、盘点确认无残留 notepad/explorer、删除 Temp 脚本）；已取得的四工作区实机证据（Processes/Ports 真实快照与详情、Containers Unavailable、File Locks Unsupported 边界态、Windows 详情无动作按钮）归档 `.omo/evidence/c3-windows-gui-partial/`（README + 5 截图）；剩余三项（File 目标内联 Unsupported、三类调查解析流、Ctrl+1..4/Escape 键位）待用户手动走查，C2/C3/C4 均保持未勾选；05/07 模块计划记录同步。教训：跨桌面键鼠注入不用于用户在场的实机验证，改人工操作 + 截图归档。
- 2026-09-10（续）：Linux GUI 交互验收（WSLg 强制 X11，`xdotool` + ImageMagick `import`）——真实产品路径走查四工作区真实快照、Partial 横幅含逐条诊断、空结果态与失败态区分、详情面板全字段、敏感值脱敏（`CLAUDE_CODE_MESSAGING_TOKEN` 已遮蔽）、中英即时重绘、浅深主题、960×640 窄窗 Sheet 与 Escape 分层、进程动作二次确认与取消（取消后目标进程存活），共 21 张留档截图；gallery 实验台验证 `Unavailable`/`Unsupported` 边界态视觉可区分。**发现并修复 3 处时间/要素缺陷**（模块 05）：确认对话框以 Rust `SystemTime` Debug 格式展示启动时间、缺少进程名与用户字段，以及 Containers 启动时间列的同源 `Debug` 渲染。修复为新增 `runquiry-ui::format::format_timestamp`（纯标准库 UTC 格式化 + 占位符，零新增依赖）供两处共用，对话框补齐总计划五要素且不改变身份冻结语义（`.omo/evidence/fix-confirm-dialog.md`；ui 78/78、工作区 394/394、clippy 零 error、GUI 实测通过，含同轮 code-review 后的重构修订）。C3 保持未勾选——Windows 侧产品路径的 Unsupported 呈现与能力运行期动态退化在 Linux 无对应场景。证据：`.omo/evidence/linux-gui-verification.md` 与 `linux-gui-verification/`。差异 #1/#2 经用户裁决**接受计划既有明示折中、记录后冻结**。

## 索引状态
- 2026-09-11T08:37:05Z：GPUI Kit 迁移局部索引同步（`@a4729f4` 工作区）；依赖及 API 路径已更新。Linux app/ui/example 测试 105/105、check/build、clippy 零 error、deny 四项通过；四工作区与实验台状态/覆盖层启动截图见 `.omo/evidence/gpui-kit-upgrade/README.md`。Windows、Wayland 与完整交互矩阵未复验；保留原全局索引基线。
- 2026-09-08 macOS 支持移除（v1 范围收窄为 Linux 与 Windows）：删除 `crates/runquiry-platform/src/macos/`（15 文件 2660 行）、`examples/macos_qa.rs`、`tests/macos_{identity,launchd,lsof}_parse.rs`、`tests/fixtures/macos/`（10 个 fixture）与模块 06 计划；app 删除 `MacosPlatform` 装配、`launchd_service_pid` 名称解析回退与 settings 的 `Library/Application Support` 分支；platform/app 增加非 Linux/Windows 目标的 `compile_error!` 门禁；core 保留 `SourceType::Launchd` / `launchd_by_pid` / `detect_launchd`（平台无关领域层，无平台实现）；`docs/witr-parity.md` 的 macOS 专属条目改标 out of scope。验证：core 109 + platform 180（合计 289）全绿、app 24 全绿、ui 68/69（唯一失败为既有测试过期，见下）。
- 上次索引：2026-09-07T14:47:43Z（标题栏合并与侧栏调宽收尾、模块文档同步，未提交工作区，主 Agent 直接增量更新）
- 上次索引：2026-09-07T13:22:30Z（Windows 主机验证通道打通 + macos_launchd_parse 搁置记录，未提交工作区，主 Agent 直接增量更新）
- 基线提交：2f93ef4（Batch 7B C4 已提交 1656fb7）
- 已知缺口：**两平台同基线全量回归均全绿**——Linux 394/394（2026-09-10）与 Windows 343/343（2026-09-11，`cfg(unix)` 差集 51 个，`.omo/evidence/c4-windows-verification.md`）；**Windows 侧全量重跑已闭合**；C4 差异 #1/#2 已裁决冻结（接受计划既有明示折中）、#3/#4 待实机复核；C3 的 Linux 侧 GUI 交互矩阵已于 2026-09-10 走查留档；Windows 侧实机走查部分完成（四工作区渲染 + Unsupported/Unavailable 边界态 + 详情无动作按钮，`.omo/evidence/c3-windows-gui-partial/`），剩 File 目标内联 Unsupported、三类调查解析流、键位/窄窗 Sheet 待**人工**走查（键鼠注入自动化已因误投递永久弃用）；C2 剩 GUI 完整交互矩阵（容器「已启动」场景无 Docker 维持不适用）；**ETXTBSY flaky 已完成根因分析**（`.omo/evidence/etxtbsy-flaky-analysis.md`：瞬态愈合实测 ≤ ~5.7ms，远小于生产 ~75ms 重试窗口，历史单次判定为环境极端尾部；C4 判定口径=再现单点失败先单跑复验再计通过，非 C4 误判源）；契约 v1 未冻结；B8 已通过（.omo/evidence/batch5-*），不重复验收
- 扫描进度：已完成
