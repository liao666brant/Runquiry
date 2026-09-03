# Runquiry Core 领域层

[crates](../) / runquiry-core

## 模块职责

Runquiry 的纯领域层：领域模型、目标解析（五类 QueryTarget）、分析管线、告警规则、刷新状态机与平台端口。A3 已落地公共领域类型与七个同步平台端口（`src/model/`、`src/port/`）；目标解析算法、来源识别、告警与分析管线属 B1（见 [模块 02 计划](../../.omo/plans/runquiry-gpui-desktop/02-core-analysis.md)）。

约束：不依赖 GPUI 或操作系统实现；平台差异通过端口（trait）抽象，由 runquiry-platform 提供实现。

## 入口与启动

无独立入口。库 crate，`src/lib.rs` 重导出 `model` 与 `port` 模块。

## 对外接口

- 领域类型（`model`）：Pid、Port、ContainerKey、QueryTarget、ProcessIdentity（身份判定唯一入口 `same_process`，executable 不参与）、ProcessSummary、ProcessDetails、ProcessAction、Renice（-20..=19）、Inspection&lt;T&gt;（部分成功）、DiagnosticIssue/DiagnosticCode、CapabilityStatus、InspectError（稳定错误码 `code()`）、SocketEntry/OpenPortEntry/ContainerSummary/FileLockEntry、CommandSpec/CommandOutput。
- 平台端口（`port`，全部同步）：ProcessInventory、ProcessDetailsProvider、NetworkInventory、ContainerInventory、FileInventory、ProcessController、CommandRunner；各 inventory 带 `capability()`。实现契约（前置/后置条件与错误语义）见各 trait 文档注释。

## 关键依赖与配置

- serde 1.0.229（derive，fixture 序列化）+ serde_json 1.0.151（dev）；manifest 继承根 workspace 的 version/edition/license/publish 与 lints。
- 行为语义的唯一来源：[docs/witr-parity.md](../../docs/witr-parity.md)（witr 行为契约，202 条）。

## 测试与质量

- `cargo test -p runquiry-core --locked`：12 个测试（tests/domain_types.rs、tests/ports.rs，含 trait 假实现消费验证）。
- lint 基线由根 `Cargo.toml` 的 `[workspace.lints]` 统一约束（unwrap/expect/panic 均 deny）。

## 常见问题

- 新增依赖必须经 A1 负责人集中修改根 workspace 配置并重新验证单一 GPUI source（见模块 01 计划的所有权边界）。

## 相关文件清单

- `crates/runquiry-core/Cargo.toml` — crate manifest（serde/serde_json）
- `crates/runquiry-core/src/lib.rs` — 库入口与模块重导出
- `crates/runquiry-core/src/model/` — 领域类型（ids/target/process/capability/diagnostic/inspection/error）
- `crates/runquiry-core/src/port/` — 平台端口 trait（process/network/container/file/command）
- `crates/runquiry-core/tests/` — 领域类型与端口集成测试
- `docs/witr-parity.md` — 领域行为契约来源
- `.omo/plans/runquiry-gpui-desktop/02-core-analysis.md` — A3/B1 任务定义

## 变更记录

- 2026-09-02：初次索引。骨架状态，仅有 manifest 与 lib.rs 占位。
- 2026-09-03：A3 落地公共领域类型、七端口与测试；新增 serde 依赖（守门人批准，零新增包）。