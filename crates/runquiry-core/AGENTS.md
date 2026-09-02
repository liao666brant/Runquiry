# Runquiry Core 领域层

[crates](../) / runquiry-core

## 模块职责

Runquiry 的纯领域层：领域模型、目标解析（五类 QueryTarget）、分析管线、告警规则、刷新状态机与平台端口。当前为骨架（A3 起逐步实现，见 [模块 02 计划](../../.omo/plans/runquiry-gpui-desktop/02-core-analysis.md)）。

约束：不依赖 GPUI 或操作系统实现；平台差异通过端口（trait）抽象，由 runquiry-platform 提供实现。

## 入口与启动

无独立入口。库 crate，`src/lib.rs` 仅有模块级文档注释（A3 前为空实现）。

## 对外接口

未发现（骨架阶段）。规划的公共领域类型见上级方案：`Pid(u32)`、`Port(u16)`、`ContainerKey`、`QueryTarget`、`ProcessIdentity`、`Inspection<T>`、`CapabilityStatus` 等。

## 关键依赖与配置

- 无第三方依赖（骨架阶段）；manifest 继承根 workspace 的 version/edition/license/publish 与 lints。
- 行为语义的唯一来源：[docs/witr-parity.md](../../docs/witr-parity.md)（witr 行为契约，202 条）。

## 测试与质量

- 测试未发现。计划命令：`cargo test -p runquiry-core --locked`（A5 引入 fixture 后）。
- lint 基线由根 `Cargo.toml` 的 `[workspace.lints]` 统一约束（unwrap/expect/panic 均 deny）。

## 常见问题

- 新增依赖必须经 A1 负责人集中修改根 workspace 配置并重新验证单一 GPUI source（见模块 01 计划的所有权边界）。

## 相关文件清单

- `crates/runquiry-core/Cargo.toml` — crate manifest（无依赖）
- `crates/runquiry-core/src/lib.rs` — 库入口（骨架）
- `docs/witr-parity.md` — 领域行为契约来源
- `.omo/plans/runquiry-gpui-desktop/02-core-analysis.md` — A3 任务定义

## 变更记录

- 2026-09-02：初次索引。骨架状态，仅有 manifest 与 lib.rs 占位。