# Runquiry UI 界面层

[crates](../) / runquiry-ui

## 模块职责

GPUI 界面层：UI 状态管理、设计系统、四个工作区（Processes、Ports、Containers、File Locks）、调查面板、设置与国际化。A4 落地设计系统（主题/状态，见 [DESIGN.md](../../DESIGN.md)）；Batch 2 B4 落地应用状态（session/shell/debounce）与 rust-i18n（locales/app.yml，删除手写 Dict，对外文案经 `tr()`）；Batch 4A B5/B6 已将真实快照、表格、筛选/排序、五类查询与详情呈现接到 app 注入的只读后端；Batch 4B B7 接通 Processes 详情的进程动作能力 seam、二次确认、异步结果门控与动作后刷新；Batch 7A C3 将能力态扩为六种呈现状态（含环境不可用边界）并落地条件 UI、能力动态门控与边界原因透出（实现完成，编译/测试/GUI 验证阻断）。

约束：只依赖 runquiry-core；禁止直接读取 /proc、调用平台 API 或运行外部命令——数据一律经 core 端口由 runquiry-platform 装配注入。

## 入口与启动

无独立入口。库 crate，`src/lib.rs` 重导出设计系统、会话、工作区与 `WorkspaceBackend` 边界。独立组件实验台见 runquiry-app 的 `examples/gallery/`。

## 对外接口

- `theme`：Runquiry Light/Dark 主题（原始色值只允许出现在 `theme.rs` 的 PALETTE 表；`install`/`apply` 切换）。
- `state` + `state_view`：DataState 七态（含 `Unavailable` 环境边界态）与 RenderOnce 统一状态呈现（loading/empty/error/unsupported/unavailable/permission-denied/ready）。
- `locale`：Lang + rust-i18n 初始化（`i18n!("locales", fallback="en")`；`extend_component_translations()` 须在 gpui_component::init 前调用一次；`set_language` 后必须显式 notify）；对外文案 API：`tr(key)`/`state_copy(state)`/`state_name(state)`（键在 `locales/app.yml`，v2 格式，en 兜底，键完整性由单测强制）。
- `session`：AppSession/WorkspaceId/WorkspaceSession——四工作区隔离的 LoadState/generation/选择/排序/筛选；手工与自动刷新共用 `try_refresh` 一条通道，in-flight 拒绝重入；刷新完成/中止与结果均绑定 generation，过期信号不改变当前请求。
- `debounce`：DetailDebounce 500ms 详情防抖（注入毫秒时钟，纯逻辑）。
- `backend`：`WorkspaceBackend` 是 UI 与装配层的采集/解析/分析及进程控制边界；`WorkspaceSnapshot` 将四类采集结果按工作区路由，`WorkspaceResultGate` 同时校验工作区与 generation，容器无已验证宿主 PID 时保留容器详情而不误报未找到；动作 seam 默认明确返回 `Unsupported`。
- `processes` / `workspaces`：进程表及 Ports、Containers、File Locks 的行模型、筛选/排序、查询结果和详情映射；`ProcessCommand` 集中声明刷新、查询、工作区与动作菜单快捷键，`ProcessActionFlow` 冻结确认时的身份、拦截重复提交并门控过期结果，具体平台操作始终由后端能力态安全禁用；C3 起能力退化经 `revoke_confirmation_if_unusable` 撤销确认、确认提交前经 `confirm_if_usable` 按当前能力再门禁（不可用时撤销并记录结构化 Unsupported 原因），清单工作区经 `LoadPresentation::boundary_reason` 保留 Unsupported/Unavailable 平台原因并以 `workspaces::interactions_enabled` 禁用边界状态下的模式/筛选交互；Processes 页表面状态由 `SurfaceState::from_parts` 推导，与清单工作区 `map_state` 共用同一套边界/失败语义（`state_from_inspection` 为单一事实源）。
- `shell`：AppShell 产品壳层（合并标题栏/侧栏四工作区/主数据区/详情区/StatusBar）；标题栏与操作栏合并为 gpui-component `TitleBar`（原生标题栏透明，窗口名与刷新/主题/语言按钮同置左侧、窗口控制自绘经 `WindowControlArea` 交系统处理），侧栏固定宽 120px；宽度 ≥1100px 使用真实 `h_resizable` 初始 65/35 主从分栏，960–1099px 保留完整主区并以选择后的 Sheet 呈现详情。进程动作区支持 TERM/KILL/STOP/CONT/renice 的 AlertDialog 二次确认、取消/关闭与结构化错误文案；动作成功按类型返回列表或保留详情并强制新代际刷新。语言切换保留会话实体，并同步既有 query/filter `InputState` placeholder；`ShellStartup` 启动设置一次到位，`ShellEvent`（主题/语言/工作区变化）交装配层持久化。真实 X11 QA 已验证五类动作、非法输入、权限边界、输入焦点及 Sheet/Dialog 分层与 Escape；合并标题栏后的 GUI 视觉矩阵验收由用户确认通过。

## 关键依赖与配置

- 依赖：`runquiry-core.workspace = true` + gpui/gpui-component（git 依赖与 runquiry-app 各自内联声明、version/rev 保持同步并经 Cargo.lock 锁定，见根 [Cargo.toml](../../Cargo.toml) 注释与根 [AGENTS.md](../../AGENTS.md)）。
- gpui-component 用法参考：`.agents/skills/gpui-component/` 与 `.agents/skills/gpui/`（含 Root 契约、element/entity/event 参考）。

## 测试与质量

- `cargo test -p runquiry-ui --locked`：Batch 4A 基线 48 个纯逻辑测试；B7 后 UI 完整套件 57/57。不要将历史数字表述为全量。**禁止添加 gpui test-support dev-dependency**（引入 deny 白名单外 git 源 proptest）。Batch 7A C3 新增 `capability_contract_tests`（7 个三平台假后端契约测试）、Batch 7B C4 新增确认门禁测试 2 个（`confirm_gate_*`）：**已随 workspace 套件在 Windows 验证主机全量运行，ui 69/69 全绿**。
- clippy 注意：锁定依赖树的 77 条 `multiple-crate-versions` 为基线既有问题，`-D warnings` 验证时豁免该项（见模块 05 计划 A4 实施记录）；ui 源码 clippy `--all-targets` 当前零 error（`surface.rs` 的 `redundant_guards` 已随合并标题栏批次修复）。
- lint 基线由根 `Cargo.toml` 的 `[workspace.lints]` 统一约束。

## 常见问题

未发现。

## 相关文件清单

- `crates/runquiry-ui/Cargo.toml` — crate manifest（含内联 git 依赖）
- `crates/runquiry-ui/src/lib.rs` — 库入口与模块重导出
- `crates/runquiry-ui/src/theme.rs` — 主题与集中式色值表
- `crates/runquiry-ui/src/state.rs` / `state_view.rs` / `locale.rs` — 状态语义/呈现/双语
- `crates/runquiry-ui/src/backend.rs` — UI 与装配层后端契约、工作区快照、结果门控与进程动作 seam
- `crates/runquiry-ui/src/processes/` / `src/workspaces/` — B5/B6 表格行模型、查询与详情映射；B7 `command.rs`、`action.rs`、`action_tests.rs` 提供动作快捷键与确认状态机
- `crates/runquiry-ui/src/session.rs` / `debounce.rs` / `shell/` — 工作区会话、详情防抖、双语 InputState 同步、响应式产品壳层及 B7 `process_actions.rs`/`render_process_actions.rs` 动作接线
- `crates/runquiry-ui/src/capability_contract_tests.rs` — C3 三平台假后端能力契约测试（`#[cfg(test)]`，复用生产状态转换）
- `crates/runquiry-app/examples/gallery/` — 独立组件实验台
- `DESIGN.md` — 唯一设计规范
- `docs/qa/a4-gallery/` — A4 视觉与键盘 QA 证据
- `.omo/plans/runquiry-gpui-desktop/05-desktop-ui.md` — A4/B4-B6 任务定义

## 变更记录

- 2026-09-02：初次索引。骨架状态，仅有 manifest 与 lib.rs 占位。
- 2026-09-03：A4 落地设计系统（主题/状态/双语占位）与 gallery 实验台；新增 gpui/gpui-component 内联 git 依赖（与 runquiry-app 同源同 rev）。
- 2026-09-03：Batch 2 B4——新增 session/shell/debounce 模块；手写 Dict 字典迁移到 rust-i18n（locales/app.yml，新增 rust-i18n 4.2.1 依赖，lock 内既有版本零新增包）；gallery 与产品壳层共用本 crate 文案 API。
- 2026-09-03：Batch 2 评审整改——删除 locales 无引用死键 `gallery.retry`。
- 2026-09-03：Batch 2 评审修复——刷新完成/中止绑定 generation；1099/1100px 断点落实；无采集器壳层改用 Unsupported 语义；窗口尺寸改由 app 层平台 bounds 事件维护。
- 2026-09-04（未提交工作区）：Batch 4A B5/B6/I1——新增只读 `WorkspaceBackend` 边界、Processes/Ports/Containers/File Locks 四工作区的真实快照与详情呈现；app 负责注入平台实现。宽窗详情改为可拖拽 `h_resizable`，窄窗使用 Sheet；长证据值允许换行。语言切换更新已创建的 query/filter InputState 占位符而不重建会话。ui 测试增至 48 个。
- 2026-09-07（未提交工作区）：Batch 4B B7——新增进程动作能力 seam、二次确认与键盘契约，确认时冻结身份并以请求/代际门控异步结果；TERM/KILL 返回列表，STOP/CONT/renice 强制刷新详情；UI 完整套件 57/57，确认态定向测试 1/1。真实 X11 QA 已覆盖五类动作、非法输入、权限边界、输入焦点及 Sheet/Dialog 分层与 Escape；B8 尚未开始，不宣称全平台验收。
- 2026-09-07（未提交工作区）：Batch 7B C4（套件准备，验证阻断）——提交前再门禁提取为 `ProcessActionFlow::confirm_if_usable`（与撤销门禁同源、复用 cancel/report 原语），壳层 `confirm_process_action` 瘦身；`action_tests.rs` 新增 2 个门禁测试。三平台契约覆盖矩阵与差异复核见 `.omo/evidence/batch7b-c4-matrix.md`；全部新代码未经编译/测试验证，C4 保持未完成、v1 契约未冻结。
- 2026-09-07（未提交工作区）：Batch 7A C3（验证阻断）——`DataState` 新增 `Unavailable` 环境边界态（DESIGN §6 六种呈现状态、图标 `info`），产品工作区状态文案统一走 `locale::workspace_state_copy`；`LoadPresentation` 保留 Unsupported/Unavailable 平台原因并在 StateView 透出，能力边界下模式/筛选/排序禁用；进程控制能力随 Processes 刷新动态取回、能力退化撤销确认并在提交前再门禁；动作错误补 `actions.error.unsupported`/`actions.error.external_tool` 专用键，删除死键 `main.collector_unavailable.*`。新增 7 个三平台假后端契约测试与 `gallery.unavailable` 相关键；全部新代码未经编译/测试/GUI 验证。code-review 修复：Processes 页失败采集不再伪装成空集合（`SurfaceState::from_parts` 与清单 `map_state` 同语义，`state_from_inspection` 改为部件签名共享单一事实源），交互门控与边界判定同源。
- 2026-09-07（未提交工作区）：Windows 验证通道打通后自动套件全绿（ui 69/69 含 C3 契约与 C4 门禁测试，clippy 零 error）；`surface.rs` 修复 deny 级 `redundant_guards`（改 `Some([])` 切片模式）。标题栏与操作栏合并为 `TitleBar`（用户请求，DESIGN §7 已同步），侧栏经多轮调宽定为 120px；GUI 视觉与交互用户确认通过。
