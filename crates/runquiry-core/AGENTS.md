# Runquiry Core 领域层

[crates](../) / runquiry-core

## 模块职责

Runquiry 的纯领域层：领域模型、目标解析（五类 QueryTarget）、分析管线、告警规则、刷新状态机与平台端口。A3 已落地公共领域类型与七个同步平台端口（`src/model/`、`src/port/`）；Batch 2 新增纯刷新策略 `src/refresh.rs`（Generation 代际 + RefreshGate 3–30s 自适应门控，无 GPUI/计时器/OS 依赖）；目标解析算法、来源识别、告警与分析管线属 B1（见 [模块 02 计划](../../.omo/plans/runquiry-gpui-desktop/02-core-analysis.md)）。

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

- `cargo test -p runquiry-core --locked`：36 个测试（lib 单测 6 + tests/domain_types.rs 11 + tests/ports.rs 1 + tests/fixtures_load.rs 14 + tests/process_controller_contract.rs 4）；tests/support/ 提供 fixture 装载器、SocketEntry 输入边界校验与假平台后端（数据在 workspace 根 tests/fixtures/，清单见其 README）。
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
- 2026-09-03：Batch 2——新增 src/refresh.rs（A5/B4 纯刷新策略与代际）；ProcessController 契约注释修正（execute 身份参数为 expected snapshot，实现须重读 current identity 并 same_process 比较，start_time None 必须拒绝）；tests/ 扩充 fixture 装载与契约测试（tests/fixtures/ 数据在 workspace 根）。
- 2026-09-03：Batch 2 评审整改——新增 `linux/file-locks-normal.json`（/proc/locks 风格 Flock+Posix）与 `linux_file_locks_fixture_loads_lock_entries`（fixtures_load 14 个）；契约测试 `from_secs(3_600)` → `from_hours(1)`（clippy 1.95 lint）。