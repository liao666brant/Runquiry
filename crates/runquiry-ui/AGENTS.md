# Runquiry UI 界面层

[crates](../) / runquiry-ui

## 模块职责

GPUI 界面层：UI 状态管理、设计系统、四个工作区（Processes、Ports、Containers、File Locks）、调查面板、设置与国际化。当前为骨架（A4 起逐步实现，见 [模块 05 计划](../../.omo/plans/runquiry-gpui-desktop/05-desktop-ui.md)）。

约束：只依赖 runquiry-core；禁止直接读取 /proc、调用平台 API 或运行外部命令——数据一律经 core 端口由 runquiry-platform 装配注入。

## 入口与启动

无独立入口。库 crate，`src/lib.rs` 仅有模块级文档注释（实现前为空）。

## 对外接口

未发现（骨架阶段）。窗口装配与 Root 契约的参考实现见 runquiry-app 的 `src/main.rs`。

## 关键依赖与配置

- 依赖：`runquiry-core.workspace = true`（workspace 继承）。
- 技术栈：Zed GPUI + gpui-component（git 依赖由 runquiry-app 内联声明并经 Cargo.lock 锁定，见根 [AGENTS.md](../../AGENTS.md)）。
- gpui-component 用法参考：`.agents/skills/gpui-component/` 与 `.agents/skills/gpui/`（含 Root 契约、element/entity/event 参考）。

## 测试与质量

- 测试未发现。GPUI 视图测试参考 `.agents/skills/gpui/references/test.md`。
- lint 基线由根 `Cargo.toml` 的 `[workspace.lints]` 统一约束。

## 常见问题

未发现。

## 相关文件清单

- `crates/runquiry-ui/Cargo.toml` — crate manifest
- `crates/runquiry-ui/src/lib.rs` — 库入口（骨架）
- `crates/runquiry-app/src/main.rs` — Root 契约与窗口装配参考
- `.omo/plans/runquiry-gpui-desktop/05-desktop-ui.md` — A4 任务定义

## 变更记录

- 2026-09-02：初次索引。骨架状态，仅有 manifest 与 lib.rs 占位。