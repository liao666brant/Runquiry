# Batch 4A / B6 独立代码审查

日期：2026-09-04

## 审查范围

- 基线：`HEAD`（未提交工作树）；只审查 `crates/runquiry-ui/src/workspaces/**`，并读取 P1
  `FileInventoryEntry` 契约及 I1 接线变更以确认模块边界。
- 需求来源：`.omo/plans/runquiry-gpui-desktop/05-desktop-ui.md` 的 B6，
  `docs/witr-parity.md` §12，以及 `DESIGN.md` §§4–8。
- 规范来源：根 `AGENTS.md`（UI 只依赖 core、真实验证、禁止无关依赖）和 Rust workspace
  lint/行数约束。
- 技能视角：已实际查阅 `remove-ai-slops` 与 `programming`。未发现为了本目标不必要的
  生产解析/归一化、无类型逃生舱、`unwrap`/`expect`/`panic`、新增依赖或超过 250 pure LOC。
  测试断言覆盖了 B6 的领域状态边界，不是删除型、常量镜像或实现自证测试。100k 测试仅锁定
  不复制领域快照这一模块责任；真实 DataTable 可见范围属于 I1/QA 条件。

## CRITICAL

- 无。

## HIGH

- 无。原先发现的壳层不可达性属于 I1 集成负责人所有，不能归因给 B6 模块 owner。

## MEDIUM

- 无。国际化文本、工具栏模式控件、TableEvent 到稳定 ID 的订阅、真实窗口滚动均为 I1/QA
  集成条件；它们保留在下文，不能视为 B6 自有模块缺陷。

## LOW

- 无。

## 验证结果

| 命令 | 结果 |
|---|---|
| `cargo test -p runquiry-ui --locked` | 通过：43 tests + 0 doc tests |
| `cargo check -p runquiry-app --locked` | 通过（初审时）；本次未重跑，后续 I1 变更后须重跑 |
| `rustfmt --edition 2024 --check crates/runquiry-ui/src/workspaces/*.rs crates/runquiry-ui/src/workspaces/tests/*.rs` | 通过 |
| `git diff --check HEAD` | 通过 |
| `cargo clippy -p runquiry-ui --locked --all-targets -- -D warnings -A clippy::multiple-crate-versions` | 通过 |

P1 `FileInventoryEntry` 的 `fd: Option<u32>` 与 `lock: Option<LockMetadata>` 明确区分普通 FD
和真实锁，Linux `open_files.rs` 也保留同 PID/path 锁优先；该契约满足 B6 所需输入，未见把普通
打开文件伪造成锁的回归。

## I1 / QA 整体验收前置（非 B6 owner 缺陷）

- I1 必须把三个 `TableState` 接入 `AppShell`，用 core/platform 结果驱动 generation，订阅
  `TableEvent` 并通过各页 `key_at` 写入稳定领域 ID；同时提供模式、筛选、排序、详情/Sheet
  和 `Inspection` 诊断渲染。
- I1 必须把列标题、单元格枚举文本和状态文本接入既有 locale；语言切换后刷新视图。
- QA 必须以真实 Linux 窗口验证 100k 表的 DataTable 可见范围/滚动、四状态矩阵、宽窄窗口、
  键盘路径和中英文截图。

## 结论

- codeQualityStatus：`CLEAR`
- recommendation：`APPROVE`
- B6 verdict：`PASS_FOR_INTEGRATION`
- blockers：无 B6 自有阻断项。
