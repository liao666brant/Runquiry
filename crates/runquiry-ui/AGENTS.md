# Runquiry UI 界面层

[crates](../) / runquiry-ui

## 模块职责

GPUI 界面层：UI 状态管理、设计系统、四个工作区（Processes、Ports、Containers、File Locks）、调查面板、设置与国际化。A4 已落地设计系统（主题/状态/双语占位，见 [DESIGN.md](../../DESIGN.md)）；应用状态、刷新、rust-i18n 与产品工作区属 B4-B6（见 [模块 05 计划](../../.omo/plans/runquiry-gpui-desktop/05-desktop-ui.md)）。

约束：只依赖 runquiry-core；禁止直接读取 /proc、调用平台 API 或运行外部命令——数据一律经 core 端口由 runquiry-platform 装配注入。

## 入口与启动

无独立入口。库 crate，`src/lib.rs` 重导出设计系统模块。独立组件实验台见 runquiry-app 的 `examples/gallery/`。

## 对外接口

- `theme`：Runquiry Light/Dark 主题（原始色值只允许出现在 `theme.rs` 的 PALETTE 表；`install`/`apply` 切换）。
- `state` + `state_view`：DataState 六态与 RenderOnce 统一状态呈现（loading/empty/error/unsupported/permission-denied/ready）。
- `locale`：Lang + Dict（en/zh-CN 最小确定性双语字典，B4 迁移 rust-i18n 前的占位）。

## 关键依赖与配置

- 依赖：`runquiry-core.workspace = true` + gpui/gpui-component（git 依赖与 runquiry-app 各自内联声明、version/rev 保持同步并经 Cargo.lock 锁定，见根 [Cargo.toml](../../Cargo.toml) 注释与根 [AGENTS.md](../../AGENTS.md)）。
- gpui-component 用法参考：`.agents/skills/gpui-component/` 与 `.agents/skills/gpui/`（含 Root 契约、element/entity/event 参考）。

## 测试与质量

- `cargo test -p runquiry-ui --locked`：10 个纯逻辑测试（主题 token 一致性、双语字典完整性、状态映射）。GPUI 视图测试参考 `.agents/skills/gpui/references/test.md`。
- clippy 注意：锁定依赖树的 77 条 `multiple-crate-versions` 为基线既有问题，`-D warnings` 验证时豁免该项（见模块 05 计划 A4 实施记录）。
- lint 基线由根 `Cargo.toml` 的 `[workspace.lints]` 统一约束。

## 常见问题

未发现。

## 相关文件清单

- `crates/runquiry-ui/Cargo.toml` — crate manifest（含内联 git 依赖）
- `crates/runquiry-ui/src/lib.rs` — 库入口与模块重导出
- `crates/runquiry-ui/src/theme.rs` — 主题与集中式色值表
- `crates/runquiry-ui/src/state.rs` / `state_view.rs` / `locale.rs` — 状态语义/呈现/双语
- `crates/runquiry-app/examples/gallery/` — 独立组件实验台
- `DESIGN.md` — 唯一设计规范
- `docs/qa/a4-gallery/` — A4 视觉与键盘 QA 证据
- `.omo/plans/runquiry-gpui-desktop/05-desktop-ui.md` — A4/B4-B6 任务定义

## 变更记录

- 2026-09-02：初次索引。骨架状态，仅有 manifest 与 lib.rs 占位。
- 2026-09-03：A4 落地设计系统（主题/状态/双语占位）与 gallery 实验台；新增 gpui/gpui-component 内联 git 依赖（与 runquiry-app 同源同 rev）。