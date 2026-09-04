# Runquiry Core 领域层

[crates](../) / runquiry-core

## 模块职责

Runquiry 的纯领域层：领域模型、目标解析（五类 QueryTarget）、分析管线、告警规则、刷新状态机与平台端口。A3 落地公共领域类型与七个同步平台端口（`src/model/`、`src/port/`）；Batch 2 新增纯刷新策略 `src/refresh.rs`（Generation 代际 + RefreshGate 3–30s 自适应门控，无 GPUI/计时器/OS 依赖）；B1 已完整落地目标解析、来源识别、告警与分析管线（`src/resolve.rs`、`src/ancestry.rs`、`src/source_detect.rs`、`src/source_shell.rs`、`src/warnings.rs`、`src/analyze.rs`，见 [模块 02 计划](../../.omo/plans/runquiry-gpui-desktop/02-core-analysis.md)）。

约束：不依赖 GPUI 或操作系统实现；平台差异通过端口（trait）抽象，由 runquiry-platform 提供实现。

## 入口与启动

无独立入口。库 crate，`src/lib.rs` 重导出 `model`、`port`、`refresh`、`resolution`、`resolve`、`ancestry`、`source_detect`、`warnings`、`analyze`。

## 对外接口

- 领域类型（`model`）：Pid、Port、ContainerKey、QueryTarget、ProcessIdentity（身份判定唯一入口 `same_process`，executable 不参与）、ProcessSummary（additive：health/container/exe_deleted/capabilities，全部 serde default）、ProcessDetails（additive：memory/io/open_files/fd_count/fd_limit）、MemoryInfo/IoStats、ProcessAction、Renice（-20..=19）、Inspection&lt;T&gt;（部分成功）、DiagnosticIssue/DiagnosticCode、CapabilityStatus、InspectError（稳定错误码 `code()`，含 `SocketOwnerUnknown`）、SocketEntry/OpenPortEntry（`validate_socket_entry` 公共边界规则）、ContainerSummary/FileLockEntry、CommandSpec/CommandOutput、SourceType/Source、HealthStatus、ContainerContext/HealthcheckStatus。
- 平台端口（`port`，全部同步）：ProcessInventory、ProcessDetailsProvider、NetworkInventory、ContainerInventory + `ContainerHealthcheckProbe`、FileInventory + `ProcessFileLocks`、ProcessController、CommandRunner、`SourceEvidenceProvider`/`SourceEvidence`（平台只采集原始证据，来源判定在 core）；各 inventory 带 `capability()`。实现契约见各 trait 文档注释。
- 目标解析与管线：`matches_exact_token`/`matches_fuzzy`/`Resolution<T>`；`resolve_name`（含 `scan_name_candidates`、`merge_service_pid`）、`resolve_port_owner`、`resolve_containers`（`ContainerMatchInput` 临时 Compose 键）、`resolve_file_holders`、`parse_pid`/`parse_port`/`parse_query`/`parse_file_path`；`resolve_ancestry`；`detect_source`；`warnings(&WarningsContext)`（§6 全部告警、顺序确定、注入时钟）；`analyze(&AnalysisPorts, ...)` 单一管线入口（部分成功、PID 复用区分退出）。

## 关键依赖与配置

- serde 1.0.229（derive，fixture 序列化）+ serde_json 1.0.151（dev）；manifest 继承根 workspace 的 version/edition/license/publish 与 lints。
- 行为语义的唯一来源：[docs/witr-parity.md](../../docs/witr-parity.md)（witr 行为契约，202 条）。

## 测试与质量

- `cargo test -p runquiry-core --locked`：104 个测试（lib 单测 7 + tests/domain_types.rs 11 + tests/ports.rs 1 + tests/fixtures_load.rs 14 + tests/process_controller_contract.rs 4 + tests/target.rs 21 + tests/pipeline.rs 30 + tests/warnings.rs 16）；tests/support/ 提供 fixture 装载器、SocketEntry 输入边界校验（委托公共 API `validate_socket_entry`）与假平台后端（数据在 workspace 根 tests/fixtures/，清单见其 README）。定向过滤器：`cargo test -p runquiry-core target|pipeline|warnings --locked`。
- lint 基线由根 `Cargo.toml` 的 `[workspace.lints]` 统一约束（unwrap/expect/panic 均 deny）。

## 常见问题

- 新增依赖必须经 A1 负责人集中修改根 workspace 配置并重新验证单一 GPUI source（见模块 01 计划的所有权边界）。

## 相关文件清单

- `crates/runquiry-core/Cargo.toml` — crate manifest（serde/serde_json）
- `crates/runquiry-core/src/lib.rs` — 库入口与模块重导出
- `crates/runquiry-core/src/model/` — 领域类型（ids/target/process/capability/diagnostic/inspection/error/source/health/container_context/resource_usage）
- `crates/runquiry-core/src/port/` — 平台端口 trait（process/network/container/file/command/source）
- `crates/runquiry-core/src/resolution.rs`、`src/resolve.rs` — 匹配纯函数、多结果容器与五类目标解析
- `crates/runquiry-core/src/ancestry.rs`、`src/source_detect.rs`、`src/source_shell.rs`、`src/warnings.rs`、`src/analyze.rs` — 祖先链、来源识别、告警与分析管线
- `crates/runquiry-core/src/refresh.rs` — generation 绑定的刷新门控与 3–30 秒自适应策略
- `crates/runquiry-core/tests/` — 领域类型、端口与 B1 算法集成测试
- `docs/witr-parity.md` — 领域行为契约来源
- `.omo/plans/runquiry-gpui-desktop/02-core-analysis.md` — A3/B1 任务定义

## 变更记录

- 2026-09-02：初次索引。骨架状态，仅有 manifest 与 lib.rs 占位。
- 2026-09-03：A3 落地公共领域类型、七端口与测试；新增 serde 依赖（守门人批准，零新增包）。
- 2026-09-03：Batch 2——新增 src/refresh.rs（A5/B4 纯刷新策略与代际）；ProcessController 契约注释修正（execute 身份参数为 expected snapshot，实现须重读 current identity 并 same_process 比较，start_time None 必须拒绝）；tests/ 扩充 fixture 装载与契约测试（tests/fixtures/ 数据在 workspace 根）。
- 2026-09-03：Batch 2 评审整改——新增 `linux/file-locks-normal.json`（/proc/locks 风格 Flock+Posix）与 `linux_file_locks_fixture_loads_lock_entries`（fixtures_load 14 个）；契约测试 `from_secs(3_600)` → `from_hours(1)`（clippy 1.95 lint）。
- 2026-09-03：Batch 2 评审修复——刷新门控保存 active generation；过期完成/中止信号不再释放新刷新或污染耗时样本，core 测试增至 37 个。
- 2026-09-03（未提交工作区）：B1 契约门——additive 公共类型与 trait 扩展：`model/source.rs`（SourceType/Source）、`model/health.rs`（HealthStatus）、`model/container_context.rs`（ContainerContext/HealthcheckStatus + cgroup 纯函数）、`model/resource_usage.rs`（MemoryInfo/IoStats）；ProcessSummary/ProcessDetails 全部 `#[serde(default)]` 扩展；`InspectError::SocketOwnerUnknown`；`validate_socket_entry` 提升为公共 API（fixture loader 委托）；`port/source.rs`（SourceEvidence/SourceEvidenceProvider）；`resolution.rs`（matches_exact_token/matches_fuzzy/Resolution）。测试增至 66 个（tests/target.rs、tests/pipeline.rs、tests/warnings.rs 起骨架）。
- 2026-09-04：B1 完整实现——`resolve.rs`（五类目标边界解析与匹配：parse_pid/parse_port/parse_query/parse_file_path、scan_name_candidates 忽略链+纯数字守卫+去重升序、resolve_name/merge_service_pid、resolve_port_owner、resolve_containers（ContainerMatchInput 临时 Compose 键）、resolve_file_holders）；`ancestry.rs`（环检测、root→target、单跳截断）；`source_detect.rs`/`source_shell.rs`（固定优先级链 + Snap/Flatpak + LXC 精化 + SSH/shell/cron/supervisor/init + unknown 兜底）；`warnings.rs`（§6 全部告警、顺序与 witr 一致、注入时钟）；`analyze.rs`（管线单一入口，单采集器失败不抹数据，NotFound/ProcessChanged 区分退出与复用）；port additive `ContainerHealthcheckProbe`/`ProcessFileLocks`；测试名统一 target_/pipeline_/warnings_ 前缀以命中定向过滤器。测试增至 104 个。
- 2026-09-04：评审修复——`resolve_ancestry` reader 升级为 `Result<Option<..>, DiagnosticIssue>`，单跳截断（权限失败/清单缺失）写入 `Inspection.issues`，截断链与完整链可区分；`analyze` 单次 `list()` 快照供祖先链与子进程快照共用（消除逐跳重复全量扫描与跨快照不一致）。测试保持 104 个（ancestry 截断诊断断言更新）。
