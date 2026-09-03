# Runquiry UI 界面层

[crates](../) / runquiry-ui

## 模块职责

GPUI 界面层：UI 状态管理、设计系统、四个工作区（Processes、Ports、Containers、File Locks）、调查面板、设置与国际化。A4 落地设计系统（主题/状态，见 [DESIGN.md](../../DESIGN.md)）；Batch 2 B4 落地应用状态（session/shell/debounce）与 rust-i18n（locales/app.yml，删除手写 Dict，对外文案经 `tr()`）；真实数据工作区属 B5/B6（见 [模块 05 计划](../../.omo/plans/runquiry-gpui-desktop/05-desktop-ui.md)）。

约束：只依赖 runquiry-core；禁止直接读取 /proc、调用平台 API 或运行外部命令——数据一律经 core 端口由 runquiry-platform 装配注入。

## 入口与启动

无独立入口。库 crate，`src/lib.rs` 重导出设计系统模块。独立组件实验台见 runquiry-app 的 `examples/gallery/`。

## 对外接口

- `theme`：Runquiry Light/Dark 主题（原始色值只允许出现在 `theme.rs` 的 PALETTE 表；`install`/`apply` 切换）。
- `state` + `state_view`：DataState 六态与 RenderOnce 统一状态呈现（loading/empty/error/unsupported/permission-denied/ready）。
- `locale`：Lang + rust-i18n 初始化（`i18n!("locales", fallback="en")`；`extend_component_translations()` 须在 gpui_component::init 前调用一次；`set_language` 后必须显式 notify）；对外文案 API：`tr(key)`/`state_copy(state)`/`state_name(state)`（键在 `locales/app.yml`，v2 格式，en 兜底，键完整性由单测强制）。
- `session`：AppSession/WorkspaceId/WorkspaceSession——四工作区隔离的 LoadState/generation/选择/排序/筛选；手工与自动刷新共用 `try_refresh` 一条通道，in-flight 拒绝重入；刷新完成/中止与结果均绑定 generation，过期信号不改变当前请求。
- `debounce`：DetailDebounce 500ms 详情防抖（注入毫秒时钟，纯逻辑）。
- `shell`：AppShell 产品壳层（侧栏四工作区/工具栏/主数据区/详情区/StatusBar）；宽度 ≥1100px 为 65/35 内联详情，960–1099px 保留完整主区并隐藏内联详情（选择后的 Sheet 交由 B5）；`ShellStartup` 启动设置一次到位，`ShellEvent`（主题/语言/工作区变化）交装配层持久化。

## 关键依赖与配置

- 依赖：`runquiry-core.workspace = true` + gpui/gpui-component（git 依赖与 runquiry-app 各自内联声明、version/rev 保持同步并经 Cargo.lock 锁定，见根 [Cargo.toml](../../Cargo.toml) 注释与根 [AGENTS.md](../../AGENTS.md)）。
- gpui-component 用法参考：`.agents/skills/gpui-component/` 与 `.agents/skills/gpui/`（含 Root 契约、element/entity/event 参考）。

## 测试与质量

- `cargo test -p runquiry-ui --locked`：21 个纯逻辑测试（主题 token 一致性、rust-i18n 双语键完整性、状态映射、会话隔离/重入/代际、防抖、1099/1100px 响应式边界）。**禁止添加 gpui test-support dev-dependency**（引入 deny 白名单外 git 源 proptest）；GPUI 交互走真实 QA。
- clippy 注意：锁定依赖树的 77 条 `multiple-crate-versions` 为基线既有问题，`-D warnings` 验证时豁免该项（见模块 05 计划 A4 实施记录）。
- lint 基线由根 `Cargo.toml` 的 `[workspace.lints]` 统一约束。

## 常见问题

未发现。

## 相关文件清单

- `crates/runquiry-ui/Cargo.toml` — crate manifest（含内联 git 依赖）
- `crates/runquiry-ui/src/lib.rs` — 库入口与模块重导出
- `crates/runquiry-ui/src/theme.rs` — 主题与集中式色值表
- `crates/runquiry-ui/src/state.rs` / `state_view.rs` / `locale.rs` — 状态语义/呈现/双语
- `crates/runquiry-ui/src/session.rs` / `debounce.rs` / `shell/` — 工作区会话、详情防抖与响应式产品壳层
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
