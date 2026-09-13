# Runquiry Core 领域层

[crates](../) / runquiry-core

## 模块职责

Runquiry 的纯领域层：领域模型、目标解析（五类 QueryTarget）、分析管线、告警规则、刷新状态机与平台端口。`src/model/` 提供值对象、诊断、部分成功与稳定错误码；`src/port/` 定义同步平台 trait（实现由 runquiry-platform 提供）；`src/resolution.rs` + `src/resolve.rs` 负责匹配与五类目标解析；`src/ancestry.rs` 负责祖先链与后代收集；`src/source_detect.rs` + `src/source_shell.rs` 负责来源识别；`src/warnings.rs` 生成 §6 告警；`src/analyze.rs` 是分析管线唯一入口；`src/refresh.rs` 是 generation 绑定的刷新门控（3–30s 自适应）。

约束：不依赖 GPUI 或操作系统实现，平台差异一律经端口抽象。`SourceType::Launchd`、`SourceEvidence::launchd_by_pid` 与 `detect_launchd` 保留为领域模型与判定链（macOS 已移出 v1 范围，当前无平台实现填充该证据）。

## 入口与启动

无独立入口。库 crate，`src/lib.rs` 声明 10 个 `pub mod`：`analyze`、`ancestry`、`model`、`port`、`refresh`、`resolution`、`resolve`、`source_detect`、`source_shell`、`warnings`，并重导出其中的公共类型；`source_shell` 是 crate 内部辅助，不在重导出列表。

## 对外接口

- 领域类型（`model`）：Pid、Port、ContainerKey、QueryTarget（ProcessName/Pid/Port/File/Container 五类）、ProcessIdentity（身份判定唯一入口 `same_process`，executable 不参与）、ProcessSummary（`identity`/`parent_pid`/`command`/`command_line`/`user` 及加性字段 `health`/`container`/`exe_deleted`/`capabilities`/`cpu_time_seconds`/`cpu_percent`/`memory_rss_bytes`/`memory_percent`，加性字段全部 `#[serde(default)]`）、ProcessDetails（`identity`/`cpu_percent`/`memory_rss_bytes`/`memory_percent`/`working_dir`/`environment`/`children` 及加性 `memory`/`io`/`open_files`/`fd_count`/`fd_limit`）、MemoryInfo/IoStats、ProcessAction（Terminate/Kill/**KillTree**/Pause/Resume/Renice，`KillTree` 为 Runquiry 扩展）、Renice（范围 -20..=19）、Inspection&lt;T&gt;（部分成功）、DiagnosticIssue/DiagnosticCode、CapabilityStatus、InspectError（8 个变体，`code()` 提供稳定键；`ExternalTool{program, detail}` 承载外部工具错误码）、SocketEntry/OpenPortEntry（`validate_socket_entry` 公共边界规则）、ContainerSummary/FileLockEntry、FileInventoryEntry/LockMetadata、CommandSpec/CommandOutput、SourceType/Source、HealthStatus、ContainerContext/HealthcheckStatus。
- 平台端口（`port`，全部同步）：ProcessInventory、ProcessDetailsProvider（字段失败以 `Inspection<ProcessDetails>` 部分成功）、NetworkInventory、ContainerInventory + `ContainerHealthcheckProbe` + `ContainerProcessVerifier`、FileInventory（`list()`/`holders(path)`）+ `ProcessFileLocks`（`locks_of(pid)`）、ProcessController、CommandRunner（含 `CancellationToken` 与超时/输出上限常量）、`SourceEvidenceProvider`/`SourceEvidence`（平台只采集原始证据，来源判定在 core）。`ProcessController` 方法：`capability`、`action_capability(&ProcessAction)`（默认回落整类能力）、`execute(&ProcessIdentity, ProcessAction)`、`reveal_capability`（默认 `Unsupported`）、`reveal_executable(&ProcessIdentity)`（默认失败）；容器 PID 候选须经 `verified_host_pid` 与平台归属证据验证。各 inventory 带 `capability()`，实现契约见 trait 文档。
- 目标解析与管线：`matches_exact_token`/`matches_fuzzy`/`Resolution<T>`；`scan_name_candidates`、`merge_service_pid`、`resolve_name`、`resolve_port_owner`（含 socket 表辅助）、`resolve_containers`（`ContainerMatchInput` 临时 Compose 键）、`resolve_file_holders`、`parse_pid`/`parse_port`/`parse_query`/`parse_file_path`；`resolve_ancestry` 与 `collect_descendants`（PPID 广度优先、环防护、不含目标自身）；`detect_source`；`warnings(&WarningsContext)`（§6 全部告警、顺序确定、注入时钟）；`analyze(target, &AnalysisPorts, now, is_windows)` 单一管线入口（部分成功、PID 复用区分退出与复用）。`Analysis` 位于 `analyze.rs`，字段为 target、ancestry、resolved_target、source、restart_count、children（**`Vec<ProcessSummary>`**，保留快照命令名供祖先树展示）、sockets、file_locks、details、warnings。

## 关键依赖与配置

- serde 1.0.229（derive）+ serde_json 1.0.151（仅 dev）；manifest 继承根 workspace 的 version/edition/license/publish 与 lints。
- 行为语义的唯一来源：[docs/witr-parity.md](../../docs/witr-parity.md)（witr 行为契约，205 条 / 12 节）。

## 数据模型

- 公共类型以加性方式演进：新增字段一律 `#[serde(default)]`，保证旧 fixture 与旧序列化数据可读；序列化兼容性由 `tests/domain_types/` 断言。
- 身份与输入边界集中在类型层：`ProcessIdentity::same_process` 是唯一身份判定入口；`validate_socket_entry` 强制 TCP/UDP 必有合法端口、Unix 必无端口，由 fixture loader 与平台实现共用。
- 值对象自带范围语义（如 `Renice` 限定 -20..=19），越界在类型构造或调用边界拒绝，不依赖下游实现。

## 测试与质量

- `cargo test -p runquiry-core --locked`：静态计数 113 个 `#[test]`——lib 内联 10（ancestry 3、refresh 7）、`tests/container_contract.rs` 3、`tests/domain_types.rs` 12、`tests/fixtures_load.rs` 13、`tests/pipeline.rs` 33、`tests/ports.rs` 1、`tests/process_controller_contract.rs` 4、`tests/target.rs` 21、`tests/warnings.rs` 16。超长集成测试已按职责拆入同名子目录（`tests/domain_types/`、`fixtures_load/`、`target/`、`pipeline/`、`warnings/`）。
- `tests/support/` 提供 fixture 装载器与假平台后端；数据在 workspace 根 `tests/fixtures/`（合成平台目录 `linux`/`windows`）。
- lint 基线由根 `Cargo.toml` 的 `[workspace.lints]` 统一约束（unwrap/expect/panic 均 deny，测试代码同样适用）。

## 常见问题

- 新增依赖必须经 A1 负责人集中修改根 workspace 配置并重新验证单一 GPUI source（见模块 01 计划的所有权边界）。

## 相关文件清单

- `crates/runquiry-core/Cargo.toml` — crate manifest（serde/serde_json）
- `crates/runquiry-core/src/lib.rs` — 模块声明与公共重导出
- `crates/runquiry-core/src/model/` — 领域类型（ids/target/process/capability/diagnostic/inspection/error/source/health/container_context/resource_usage）
- `crates/runquiry-core/src/port/` — 平台端口 trait（process/network/container/file/command/source）
- `crates/runquiry-core/src/analyze.rs` — 分析管线入口与 `Analysis`
- `crates/runquiry-core/src/ancestry.rs` — 祖先链解析 `resolve_ancestry` 与后代收集 `collect_descendants`
- `crates/runquiry-core/src/resolution.rs`、`src/resolve.rs` — 匹配纯函数、多结果容器与五类目标解析
- `crates/runquiry-core/src/source_detect.rs`、`src/source_shell.rs` — 来源识别固定优先级链与 shell 辅助
- `crates/runquiry-core/src/warnings.rs` — §6 告警规则
- `crates/runquiry-core/src/refresh.rs` — generation 绑定的刷新门控与 3–30 秒自适应策略
- `crates/runquiry-core/tests/` — 领域类型、容器 PID 契约、端口与算法集成测试（按同名子目录拆分）
- `docs/witr-parity.md` — 领域行为契约来源
- `.omo/plans/runquiry-gpui-desktop/02-core-analysis.md` — A3/B1 任务定义
