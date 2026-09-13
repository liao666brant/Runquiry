# Runquiry UI 界面层

[crates](../) / runquiry-ui

## 模块职责

GPUI 界面层：UI 状态管理、设计系统、四个工作区（Processes、Ports、Containers、File Locks）、调查面板、设置与国际化。`src/theme.rs` 提供浅/深主题，`src/state.rs` + `src/state_view.rs` 统一七种呈现态，`src/session.rs` + `src/debounce.rs` 管理工作区会话与详情防抖，`src/processes/` 与 `src/workspaces/` 承载四个工作区的行模型、表格与详情，`src/shell/` 是产品壳层与全部交互接线，`src/backend.rs` 是 UI 与装配层的后端边界，`src/locale.rs` + `locales/app.yml` 提供双语文案。

约束：只依赖 runquiry-core；禁止直接读取 /proc、调用平台 API 或运行外部命令——数据一律经 core 端口由 runquiry-app 装配注入。

## 入口与启动

无独立入口。库 crate，`src/lib.rs` 声明并重导出 `backend`、`debounce`、`format`、`locale`、`processes`、`session`、`shell`、`state`、`state_view`、`theme`、`workspaces` 十个公开模块，另以 `#[cfg(test)]` 引入 `capability_contract_tests`；`rust_i18n::i18n!("locales", fallback = "en")` 在 crate 内初始化。独立组件实验台见 runquiry-app 的 `examples/gallery/`。

## 对外接口

- `theme`：Runquiry Light/Dark 主题（原始色值只允许出现在 `theme.rs` 的 PALETTE 表；`install`/`apply` 切换）。
- `state` + `state_view`：`DataState` 七变体（`Ready` + `Loading`/`Empty`/`Error`/`Unsupported`/`Unavailable`/`PermissionDenied` 六种呈现态）与 `RenderOnce` 统一状态呈现（`StateView::new/description/action/note`）；`ALL`/`key`/`parse`/`icon` 供 CLI 与 gallery 共用。
- `format`：用户可读格式化与统一占位符。`format_timestamp(Option<SystemTime>)` 输出 UTC `YYYY-MM-DD HH:MM:SS UTC`，`format_bytes(u64)` 以 1024 基数输出 B…EB，`format_optional(Option<&str>)` 与二者缺失时统一回退 `UNAVAILABLE`（`—`）。纯标准库实现，供确认对话框、进程表与容器表共用，避免 `Debug` 打印 `SystemTime` 或各自定义占位符。
- `locale`：`Lang`（En/ZhCn）+ rust-i18n 初始化（`extend_component_translations()` 须在 `gpui_kit::init` 前调用一次；`set_language` 后必须显式 notify）；对外文案 API：`tr(key)`、`state_copy(state)`、`state_name(state)`、`workspace_state_copy(state)`；键在 `locales/app.yml`（v2 格式，en 兜底）。
- `session`：`WorkspaceId`（四工作区）+ `WorkspaceSession`（LoadState/generation/选择/排序/筛选；手工与自动刷新共用 `try_refresh` 一条通道，in-flight 拒绝重入；完成/中止与结果均绑定 generation，过期信号不改变当前请求）+ `AppSession`（工作区隔离与切换）。
- `debounce`：`DetailDebounce` 500ms 详情防抖（注入毫秒时钟，纯逻辑，每个请求只交付一次）。
- `backend`：`WorkspaceBackend` 是 UI 与装配层的采集/解析/分析及进程控制边界，方法为 `load`/`resolve`/`analyze`/`process_control_capability`/`process_action_capabilities`/`execute_process_action`/`reveal_process_executable`（后两者与动作能力默认返回 `Unsupported`）；`ProcessActionCapabilities` 含 `class`/`terminate`/`kill`/`kill_tree`/`pause`/`resume`/`renice`/`reveal` 八个能力位（`all_unavailable` 一并置 Unavailable）；`WorkspaceSnapshot` 四变体各带 `capability` 与 `Inspection`，`WorkspaceResultGate` 同时校验工作区与 generation。
- `processes`：进程行模型、查询、脱敏与虚拟化表格；`ProcessTableDelegate::column_defs()` 固定八列（名称 180 / PID 88 / CPU% 88 / 内存 160 / 启动时间 180 / 用户 140 / 健康 110 / 命令 280）——名称列渲染短名 `command`，命令列渲染完整 `command_line`（缺失回退短名并带 tooltip）；排序为表头点击（`ProcessSortKey::{Pid,Cpu,Memory}` 的列 `.sortable()`，箭头由当前排序状态渲染），默认 CPU% 降序，`ColumnSort::Default` 回落默认；`ProcessSort::apply` 中排序方向只作用于有值键，`None` 行恒排尾部（PID 升序），同值行 PID 升序不随方向反转；筛选为单一输入框即时匹配 command/command_line/PID/user。右键菜单在关闭类（`actions.kill`、`actions.kill_tree`，携带右键行身份回发壳层）之后加分隔线与「打开文件位置」（`actions.reveal_executable`），可见性由能力快照控制。`ProcessActionFlow` 冻结确认时的身份、拦截重复提交并门控过期结果，方法为 `new`/`generation`/`is_busy`/`is_confirming`/`is_executing`/`last_error`/`report_error`/`report_presentation_result`/`invalidate_context`/`request`/`cancel_confirmation`/`revoke_confirmation_if_unusable`/`confirm_if_usable`/`confirm`/`complete`；`ProcessCommand` 集中声明刷新、查询、工作区与动作菜单快捷键。
- `workspaces`：Ports、Containers、File Locks 三工作区的行模型、表格、共享加载与详情映射；`common.rs` 提供 `LoadPresentation`（含 `boundary_reason` 保留 Unsupported/Unavailable 平台原因）、`StableSelection`、`ColumnVisibility`（隐藏列集合 `BTreeSet<String>`，`new`/`set_hidden`/`hidden`/`set_visible`/`visible`/`visible_count`）与 `interactions_enabled`（边界状态下禁用模式/筛选交互）；四个表格 delegate 均有 `set_hidden`/`visible_columns`/`column_defs`，可见索引→列 key 分派，隐藏不影响后续列渲染与表头排序。Processes 页表面状态由 `SurfaceState::from_parts` 推导，与清单工作区 `map_state` 共用同一套边界/失败语义（`state_from_inspection` 为单一事实源）。
- `shell`：`AppShell` 产品壳层（合并标题栏 / 侧栏四工作区 / 主数据区 / 详情区 / StatusBar）。标题栏为 `gpui_kit::component::TitleBar`（原生标题栏透明，窗口名与刷新/主题/语言按钮同置左侧，窗口控制经 `WindowControlArea` 自绘交系统）；侧栏宽由 `shell::render::SIDEBAR_WIDTH = 120` 单一来源定义；`width >= 1100px` 使用真实 `h_resizable` 初始 65/35 主从分栏，960–1099px 主区保持完整并以 Sheet 呈现详情（Escape 分层关闭）。调查栏第一行为五类目标按钮 + 「容器」之后的列设置图标按钮（`IconName::Settings2` + `Popover` + `Checkbox`，实时读壳层状态并经 `Entity<AppShell>` 派发），第二行为单一查询输入框（`InputEvent::Change` 即时筛选、回车发起调查）；进程动作区支持 TERM/KILL/STOP/CONT/renice 的 AlertDialog 二次确认、取消与结构化错误文案；祖先树以等宽字体渲染（祖先链 `└─` 缩进、子进程 `├─`/`└─` 同层、超过 10 个子进程折叠为 `detail.ancestry.more` 计数、不做行数截断）。壳层用四个独立任务槽（`refresh_task`/`detail_task`/`process_action_task`/`reveal_task`），其中「打开文件位置」经独立 `reveal_process_executable` 执行，结果走 `ProcessActionFlow::report_presentation_result` 只写错误槽，不触碰 `pending`/`in_flight`。
- 设置相关：`ShellStartup` 一次性注入启动设置（theme/language/workspace/hidden_columns），`ShellEvent`（主题/语言/工作区/隐藏列变化）交装配层持久化。

## 关键依赖与配置

- `runquiry-core.workspace = true` + `gpui-kit.workspace = true`（版本由根 [Cargo.toml](../../Cargo.toml) 精确锁定）；`rust-i18n 4.2.1`（locales 双语）。组件使用 `gpui_kit::component`，GPUI 类型使用 `gpui_kit` 重导出。
- 无 dev-dependencies；`[lints] workspace = true`。
- GPUI Kit API 与框架机制参考 `.agents/skills/gpui-kit/`；设计与交互变更先读 `.agents/skills/gpui-kit-design-guides/`。每个窗口第一级视图必须是 `gpui_kit::component::Root`。

## 测试与质量

- `cargo test -p runquiry-ui --locked`：静态计数 93 个 `#[test]`——`src/capability_contract_tests.rs` 7（独立测试模块，复用生产状态转换的平台风格假后端契约）、`src/processes/action_tests.rs` 12、`src/processes/tests.rs` 11、`src/workspaces/tests/` 16（common 5 / containers 3 / file_locks 4 / ports 4）、`src/format.rs` 10，其余为各模块内联测试（session 7、backend 5、shell/data 4，state/theme/debounce/locale/workspaces-common/shell-render 各 3，shell/process_actions 2，shell/refresh 1）。
- 禁止添加 gpui test-support dev-dependency；纯逻辑测试一律普通 `#[test]`，新增测试依赖须经依赖守门人核验。
- 文案键完整性由 `locale::tests::both_languages_have_the_same_keys` 强制（en 与 zh-CN 键集合与空值比对），另有 `languages_differ` 与 `lang_round_trips_and_switches_locale`；`format`、`session`、`ProcessSort`、`ColumnVisibility`、`SurfaceState` 等纯逻辑均有内联测试。
- lint 基线由根 `Cargo.toml` 的 `[workspace.lints]` 统一约束；本 crate 只有两处针对性 `cast_precision_loss` 豁免（`format.rs`、`shell/data.rs`），无 crate 级 clippy 豁免。
- 视觉与键盘验收证据在 `docs/qa/a4-gallery/`；产品壳层的 Linux 与 Windows 实机走查记录见 `.omo/evidence/`（未入 Git）。

## 常见问题

- 新增或修改对外文案必须同时补齐 `locales/app.yml` 的 en 与 zh-CN 两组键，否则 `both_languages_have_the_same_keys` 失败；状态文案统一走 `locale::workspace_state_copy`，不要新增按工作区散落的键。
- 原始色值只允许出现在 `theme.rs` 的 PALETTE 表；其余渲染代码取主题 token。

## 相关文件清单

- `crates/runquiry-ui/Cargo.toml` — crate manifest（继承 workspace GPUI Kit 依赖）
- `crates/runquiry-ui/src/lib.rs` — 模块声明、公共重导出与 i18n 宏
- `crates/runquiry-ui/src/theme.rs` — 主题与集中式色值表
- `crates/runquiry-ui/src/state.rs`、`src/state_view.rs`、`src/locale.rs`、`locales/app.yml` — 状态语义、统一呈现与双语
- `crates/runquiry-ui/src/format.rs` — 时间/字节/占位符的唯一格式化实现
- `crates/runquiry-ui/src/backend.rs` — UI 与装配层后端契约、能力位、工作区快照与结果门
- `crates/runquiry-ui/src/session.rs`、`src/debounce.rs` — 工作区会话代际与详情防抖
- `crates/runquiry-ui/src/processes/` — 进程行模型、查询、脱敏、八列表格（`table.rs`）、动作状态机（`action.rs`）与 `SurfaceState`（`surface.rs`）
- `crates/runquiry-ui/src/workspaces/` — 三工作区行模型与表格、`common.rs` 共享加载/选择/列显隐，`tests/` 为对应纯逻辑测试
- `crates/runquiry-ui/src/shell/` — `mod.rs`（AppShell 状态与 ShellEvent）、`data.rs`（可见状态、筛选/排序、列显隐）、`process_actions.rs` + `refresh.rs`（动作、reveal 与能力动态刷新）、`render*.rs`（标题栏、调查栏/表格、详情、证据与祖先树）
- `crates/runquiry-ui/src/capability_contract_tests.rs` — 平台风格假后端能力契约测试（`#[cfg(test)]`）
- `crates/runquiry-app/examples/gallery/` — 独立组件实验台
- `DESIGN.md` — 唯一设计规范
- `docs/qa/a4-gallery/` — A4 视觉与键盘 QA 证据
- `.omo/plans/runquiry-gpui-desktop/05-desktop-ui.md` — A4/B4–B7 任务定义
