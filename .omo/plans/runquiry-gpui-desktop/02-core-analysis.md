# 模块 02：核心领域与调查分析

上级方案：[Runquiry 桌面化实施计划](../runquiry-gpui-desktop.md)

## 模块目标

实现与 UI、操作系统无关的领域模型、目标解析和调查分析管线，使所有平台共享同一套匹配、来源、告警、部分结果和错误语义。

## 所有权与边界

允许写入：

- crates/runquiry-core/src/
- crates/runquiry-core/tests/

不得写入：

- runquiry-platform、runquiry-ui、runquiry-app。
- 根 Cargo 配置和 Cargo.lock。
- 任何直接系统调用或 GPUI 类型。

需要新依赖时向 A1 负责人提出需求，不直接修改共享 manifest。

## TODOs

- [x] **A3 核心领域接口**
  - 依赖：A1、A2。
  - 先以编译失败测试锁定上级方案定义的 Pid、Port、ContainerKey、QueryTarget、ProcessIdentity、Inspection、CapabilityStatus、InspectError 和 ProcessAction。
  - 定义 ProcessInventory、ProcessDetailsProvider、NetworkInventory、ContainerInventory、FileInventory、ProcessController、CommandRunner。
  - traits 保持同步和职责单一；平台调用由上层后台执行，不在 core 引入 async runtime。
  - 公共结果使用结构化字段和稳定错误码，不以自由字符串承担控制流。
  - 核心模型可序列化为测试 fixture，但不定义持久化数据库格式。
  - 验证：
    - cargo test -p runquiry-core --locked
    - cargo clippy -p runquiry-core --all-targets -- -D warnings
  - 完成证据：公共 API 清单、依赖树中不存在 GPUI/OS crate、测试与 Clippy 结果。
  - 实施记录（2026-09-03）：
    - TDD：先写 tests/domain_types.rs 与 tests/ports.rs 观察编译失败（E0432），再实现转绿。
    - 公共类型：Pid（拒绝 0）、Port（1-65535）、ContainerKey（dedup_key 为 runtime|id）、QueryTarget（五变体）、ProcessIdentity、ProcessSummary、ProcessDetails、ProcessAction、Renice（-20..=19，TryFrom 校验）、Inspection&lt;T&gt;（data: Option&lt;T&gt; + issues + captured_at，支持部分成功）、DiagnosticIssue/DiagnosticCode、CapabilityStatus（四态）、InspectError（七变体）、SocketEntry（含 remote_addr: Option&lt;String&gt;、Protocol 含 Unix）、OpenPortEntry、ContainerSummary、FileLockEntry/LockType/LockMode、CommandSpec/CommandOutput；常量 PROBE_TIMEOUT=500ms、LIST_TIMEOUT=3s、DETAIL_TIMEOUT=5s、STDOUT/STDERR_LIMIT=8MiB。
    - 稳定错误码：invalid_target / not_found / ambiguous / permission_denied / unsupported / external_tool / process_changed（InspectError）；permission_denied / external_tool_failed / timeout / output_limit_exceeded / unsupported / platform_unavailable / parse_failed / unknown（DiagnosticCode）。自由文本仅入 message（展示用），不承担控制流。
    - 身份语义：ProcessIdentity 以 same_process()（pid + start_time 且 start_time 为 Some）为唯一身份判定入口；executable 不参与判定；两侧 start_time 均 None 视为不可验证（拒绝破坏性操作）。未派生 PartialEq 以杜绝复用防护旁路。ProcessController 契约要求执行前用 same_process 校验重读身份。
    - 七个 trait 全部同步，各 inventory 带 capability() -> CapabilityStatus；文件均在 250 行内（最大 model/process.rs 184 行）。
    - 依赖批准记录（依赖守门人，2026-09-03）：runquiry-core 新增 serde 1.0.229（derive）与 serde_json 1.0.151（dev）——两者均已在 Cargo.lock 中作为 GPUI 传递依赖存在，零新增包；Cargo.lock 变更仅为 runquiry-core/runquiry-ui 条目的依赖列表更新，Zed GPUI（23 包，f66ed399）与 gpui-component（4 包，91217366）source 未漂移，cargo deny check 全绿。
    - 验证结果：cargo fmt --all --check 通过；cargo test 12 个测试全过；cargo clippy --all-targets -- -D warnings 零警告（无豁免）；cargo tree -p runquiry-core --locked 无 gpui/OS API/tokio/async-std/smol/futures。
    - 接口冻结范围裁定：parity §1 中的容器上下文（ContainerID/ContainerRuntime/ContainerHealthcheck）、启动来源 Source、健康状态枚举、ExeDeleted、Capabilities、MemoryInfo/IOStats、FileContext，以及 FileInventory 的全量打开文件列举，推迟到 B1/B2 随分析管线一并实现（避免 A3 投机性建字段）；B1/B2 扩展时按 additive 演进。
    - 对平台实现者的前置/后置条件与错误语义见 crates/runquiry-core/src/port/ 各 trait 契约文档（单条目失败只追加 issue 不丢数据、无主端口 pid: None + issue、运行时缺失用 Unavailable、执行前重读身份否则 ProcessChanged、超时/程序缺失归 ExternalTool、输出截断置 *_truncated）。

- [x] **B1 目标解析与分析管线**
  - 依赖：A3、A5。
  - Name：大小写不敏感模糊匹配；exact 时匹配进程名、完整命令参数或路径段的完整 token。
  - PID：只接受正整数。
  - Port：只接受 1-65535。
  - File：保留输入路径语义，由 FileInventory 解析使用者。
  - Container：按名称、镜像、命令、Compose project/service 匹配；exact 时要求字段完全相等。
  - 多结果返回 Ambiguous 和完整候选，不自动选择第一项。
  - 组合进程、祖先链、来源、容器、资源、Socket、文件和告警，允许子采集器部分失败。
  - 按上级方案维护来源优先级和告警纯函数。
  - 使用 ProcessIdentity 区分“进程退出”和“PID 被复用”。
  - 验证：
    - cargo test -p runquiry-core target --locked
    - cargo test -p runquiry-core pipeline --locked
    - cargo test -p runquiry-core warnings --locked
  - 完成证据：五类目标矩阵、来源/告警 fixture 对照、部分成功和 PID 复用测试结果。
  - 实施记录（2026-09-04）：
    - 契约门（additive，接口已冻结）：`model/source.rs`（SourceType 11 变体/Source 有序 details）、`model/health.rs`（HealthStatus）、`model/container_context.rs`（ContainerContext/HealthcheckStatus + cgroup v1/v2 纯函数）、`model/resource_usage.rs`（MemoryInfo/IoStats）；ProcessSummary/ProcessDetails 全部 `#[serde(default)]` 扩展（30 个既有 fixture 逐字节未改）；`InspectError::SocketOwnerUnknown`；`validate_socket_entry` 提升为公共 API（fixture loader 委托复用）；`port/source.rs`（SourceEvidence/SourceEvidenceProvider：平台只采集原始证据、core 做全部判定）；`resolution.rs`（matches_exact_token/matches_fuzzy/Resolution）。
    - B1 实现（全在 core，零 OS/GPUI/外部命令依赖）：`resolve.rs`（parse_pid/parse_port/parse_query/parse_file_path 边界解析；scan_name_candidates 忽略链+纯数字守卫+去重升序；resolve_name（NotFound/Ambiguous+完整候选；merge_service_pid 服务 PID 首位合并）；resolve_port_owner（SocketOwnerUnknown/多属主 Ambiguous）；resolve_containers（五字段匹配含 Compose 临时键、runtime+id 去重）；resolve_file_holders）；`ancestry.rs`（resolve_ancestry：环检测、root→target、单跳截断部分成功）；`source_detect.rs` + `source_shell.rs`（固定优先级链 container→ssh→shell→systemd→launchd→bsdrc→supervisor→cron→windows_service→init→unknown，Snap/Flatpak、LXC runtime 精化、SSH env 回溯、multiplexer 富化）；`warnings.rs`（§6 全部告警，顺序与 witr append 顺序逐条一致，可注入时钟 `now`，LD_PRELOAD 先于排序后的 DYLD_*）；`analyze.rs`（单一入口 analyze：祖先→来源→健康检查补全→详情/子进程/Socket/文件锁收集→告警→Analysis，单采集器失败不抹数据，NotFound/ProcessChanged 区分退出与 PID 复用）；`port/container.rs` additive `ContainerHealthcheckProbe`、`port/file.rs` additive `ProcessFileLocks`。
    - 行为锁定以测试内联 JSON 与 core tests/support 的假采集器实现（tests/fixtures/ 未新增文件，fixture 对照语义由既有 30 个 fixture + 测试内联合成数据共同覆盖）。
    - 验证：`cargo test -p runquiry-core target|pipeline|warnings --locked` 23/30/16 通过；core 合计 104（lib 7 + domain 11 + fixtures 14 + pipeline 30 + ports 1 + controller 4 + target 21 + warnings 16）；`cargo tree -p runquiry-core --locked` 无 GPUI/OS crate/async runtime；clippy `-D warnings` 零警告。
    - 已知限制：systemd 服务回退的 systemctl 调用属平台/上层（core 仅提供 merge_service_pid 组合语义）；launchd/Windows SCM 判定保留类型与链位置，平台探测属 C1/C2。
    - 评审修复（2026-09-04）：`resolve_ancestry` 的 reader 升级为 `Result<Option<..>, DiagnosticIssue>`——单跳截断的失败原因（权限/快照失败）与清单缺失均写入 `Inspection.issues`，截断链与完整链可区分（根计划完成标准「部分结果可区分」）；`analyze` 改为单次 `list()` 快照，祖先链与子进程快照共用同一份（消除逐跳全量重复扫描与跨快照不一致）。

## 模块退出条件

- core 不依赖 GPUI、平台 crate 或外部命令。
- 五类目标和所有告警均由 fixture 锁定。
- 平台模块只需实现 traits，不需要重复业务判断。

## 交接格式

- 公共类型与 trait 列表。
- 对平台实现者必须满足的前置/后置条件。
- 对 UI 暴露的状态与错误码。
- 自动验证命令和结果。
