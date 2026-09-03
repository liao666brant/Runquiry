# A5 Fixture 与测试基建（workspace 根 `tests/fixtures/`）

对 [witr 行为契约](../docs/witr-parity.md)（§1 领域模型、§8 刷新、§9 进程操作、§10 平台差异）
的三平台合成 fixture、确定性测试基建与失败模式映射。**全部数据为合成值**：
进程名一律 `fxt-` 前缀，用户名 `fixture-user`，路径 `/opt/runquiry-fixtures/…`
（Windows `C:\opt\runquiry-fixtures\…`，macOS `/Users/fixture-user/Library/RunquiryFixtures/…`），
容器 ID `fxt…`，采集时刻固定为 epoch 毫秒 `1700000000000`，generation 固定 `7`。

## 1. fixture 清单（平台 / 场景 / 领域类型）

封套格式：`platform`、`scenario`、`captured_at_epoch_ms`、`generation`、
可选 `capability`（`CapabilityStatus`）、`data`（领域类型或 `null`）、`issues`
（`DiagnosticIssue` 列表）。装载器见
`crates/runquiry-core/tests/support/fixtures.rs`。

| 文件 | 平台 | 场景 | 领域类型 | 说明 |
|---|---|---|---|---|
| `linux/processes-normal.json` | linux | 正常 | `Vec<ProcessSummary>` | 双进程基线，`/proc` 风格合成路径 |
| `linux/processes-empty.json` | linux | 空结果 | `Vec<ProcessSummary>` | `data = []`，非失败 |
| `linux/processes-partial.json` | linux | 部分成功 | `Vec<ProcessSummary>` | 数据 + `permission_denied` 诊断 |
| `linux/processes-permission.json` | linux | 权限失败 | `Vec<ProcessSummary>` | `data = null` + `permission_denied` |
| `linux/containers-tool-missing.json` | linux | 工具缺失 | `Vec<ContainerSummary>` | docker 数据 + podman 缺失 = 部分成功；`capability: Partial` |
| `linux/open-ports-timeout.json` | linux | 超时 | `Vec<OpenPortEntry>` | 整体超时，`data = null` |
| `linux/sockets-normal.json` | linux | 正常 | `Vec<SocketEntry>` | Tcp/Tcp6/Udp，条目带 inode |
| `linux/sockets-malformed.json` | linux | 格式损坏 | `Vec<SocketEntry>` | JSON 合法但 TCP 条目 `port = 0` → 反序列化拒绝 |
| `linux/pid-reuse.json` | linux | PID 复用 | `Vec<ProcessIdentity>` | 同 PID 三身份：T1 / T2 / `start_time: null` |
| `linux/file-locks-normal.json` | linux | 正常 | `Vec<FileLockEntry>` | `/proc/locks` 风格：Flock + Posix 各一条（parity §1） |
| `macos/processes-normal.json` | macos | 正常 | `Vec<ProcessSummary>` | `/Users/fixture-user/...` 合成路径 |
| `macos/processes-empty.json` | macos | 空结果 | `Vec<ProcessSummary>` | 同 linux |
| `macos/processes-partial.json` | macos | 部分成功 | `Vec<ProcessSummary>` | 同 linux |
| `macos/processes-permission.json` | macos | 权限失败 | `Vec<ProcessSummary>` | 同 linux |
| `macos/containers-tool-missing.json` | macos | 工具缺失 | `Vec<ContainerSummary>` | 唯一运行时缺失 = 完全失败；`capability: Unavailable` |
| `macos/open-ports-timeout.json` | macos | 超时 | `Vec<OpenPortEntry>` | lsof 超时但保留已取得端口 = 部分成功 |
| `macos/file-locks-normal.json` | macos | 正常 | `Vec<FileLockEntry>` | lsof best-effort 文件锁 |
| `macos/sockets-normal.json` | macos | 正常 | `Vec<SocketEntry>` | 含 Unix socket（`port: null`，inode 为 `null`） |
| `macos/sockets-malformed.json` | macos | 格式损坏 | `Vec<SocketEntry>` | JSON 被截断 → 解析层拒绝 |
| `macos/pid-reuse.json` | macos | PID 复用 | `Vec<ProcessIdentity>` | 同 linux |
| `windows/processes-normal.json` | windows | 正常 | `Vec<ProcessSummary>` | `C:\opt\runquiry-fixtures\...` 合成路径 |
| `windows/processes-empty.json` | windows | 空结果 | `Vec<ProcessSummary>` | 同 linux |
| `windows/processes-partial.json` | windows | 部分成功 | `Vec<ProcessSummary>` | 同 linux |
| `windows/processes-permission.json` | windows | 权限失败 | `Vec<ProcessSummary>` | 同 linux |
| `windows/containers-tool-missing.json` | windows | 工具缺失 | `Vec<ContainerSummary>` | 完全失败 + `capability: Unavailable` |
| `windows/open-ports-timeout.json` | windows | 超时 | `Vec<OpenPortEntry>` | 整体超时 |
| `windows/sockets-normal.json` | windows | 正常 | `Vec<SocketEntry>` | Tcp/Udp，无 inode，含无主端口（`owner_pid: null`） |
| `windows/sockets-malformed.json` | windows | 格式损坏 | `Vec<SocketEntry>` | JSON 合法但 Unix 条目携带端口 → 边界规则拒绝 |
| `windows/pid-reuse.json` | windows | PID 复用 | `Vec<ProcessIdentity>` | 同 linux |
| `windows/file-locks-unsupported.json` | windows | 能力不支持 | `Vec<FileLockEntry>` | `capability: Unsupported`，不返回伪数据（parity §10） |

## 2. 失败模式映射（八类场景 → fixture → 测试）

| 场景 | fixture 文件 | runquiry-core 测试（`tests/fixtures_load.rs` 等） | runquiry-platform 测试（`tests/fake_backends.rs`） |
|---|---|---|---|
| 正常 | `{平台}/processes-normal.json`、`{平台}/sockets-normal.json`、`{linux,macos}/file-locks-normal.json` | `processes_normal_fixture_loads_for_all_platforms`、`sockets_normal_fixture_passes_boundary_validation_for_all_platforms`、`{linux,macos}_file_locks_fixture_loads_lock_entries` | `normal_scenario_provides_data_on_all_ports` |
| 空结果 | `{平台}/processes-empty.json` | `processes_empty_fixture_yields_complete_empty_data` | `empty_scenario_yields_complete_empty_collections` |
| 部分成功 | `{平台}/processes-partial.json`、`linux/containers-tool-missing.json`、`macos/open-ports-timeout.json` | `processes_partial_fixture_keeps_data_alongside_permission_issue`、`containers_tool_missing_fixture_reports_external_tool_and_capability`、`open_ports_timeout_fixture_reports_timeout` | `partial_scenario_keeps_data_alongside_issue`、`tool_missing_scenario_reports_external_tool_and_unavailable_capability` |
| 权限失败 | `{平台}/processes-permission.json` | `processes_permission_fixture_fails_without_data` | `permission_scenario_fails_without_data` |
| 工具缺失 | `{平台}/containers-tool-missing.json` | `containers_tool_missing_fixture_reports_external_tool_and_capability` | `tool_missing_scenario_reports_external_tool_and_unavailable_capability` |
| 超时 | `{平台}/open-ports-timeout.json` | `open_ports_timeout_fixture_reports_timeout` | `timeout_scenario_reports_timeout_issue_and_runner_error` |
| 格式损坏 | `{平台}/sockets-malformed.json`（三种损坏层：反序列化 / 解析 / 边界规则） | `sockets_malformed_fixtures_are_rejected_by_loader`、`socket_boundary_rule_rejects_inline_violations` | `malformed_runner_output_fails_consumer_parse` |
| PID 复用 | `{平台}/pid-reuse.json` | `pid_reuse_fixture_provides_distinguishable_identities`、`process_controller_contract.rs` 的 4 个测试 | `controller_rejects_reused_pid_and_unverifiable_identity_without_side_effect` |

平台能力差异（parity §10）另由 `windows_file_locks_fixture_marks_capability_unsupported`
（core）与 `capability_status_is_distinguishable_per_scenario`（platform）覆盖。

## 3. 确定性测试基建

- **确定性时钟** `FixedClock`（`crates/runquiry-core/tests/support/mod.rs` 与
  `crates/runquiry-platform/tests/support/mod.rs` 各一份，语义相同）：现在时刻由
  测试显式给出（相对 `UNIX_EPOCH` 的毫秒数，与 fixture 的 `captured_at_epoch_ms`
  同值约定），经 `Inspection::with_captured_at` 注入快照；**不读 `SystemTime::now`
  墙钟，无真实 sleep**。
- **固定 generation** `Generation::FIXTURE = 7`：与全部 fixture 封套的 `generation`
  字段一致，测试断言「数据属于哪一代请求」；不依赖任何全局可变状态。
- **假平台后端**：core 侧 `FakeController`（`tests/support/fakes.rs`）只实现
  `ProcessController`；platform 侧 `FakePlatform`（`tests/support/fakes.rs`）一个
  结构实现全部七个端口 trait，是失败注入入口（按 `Scenario` 返回对应数据/错误）。
- **双份维护说明**：core 与 platform 的测试 crate 无法共享模块（跨 crate 测试
  代码不可见），时钟/generation 刻意双份、同语义；若语义调整须两处同步。
- 测试不依赖真实进程、真实容器 CLI、真实 lsof 或宿主机网络状态。

## 4. 输入边界校验（后续平台真实输入必须复用）

`crates/runquiry-core/tests/support/fixtures.rs` 的 `validate_socket_entries`：

- `Tcp` / `Tcp6` / `Udp` / `Udp6` 条目：`port` 必须为 `Some` 且落在 1..=65535
  （`Port` 新类型已在反序列化层拒绝 0，此处显式表达「必须有端口」）；
- `Unix` 条目：`port` 必须为 `None`（Unix domain socket 无端口语义，不得编造）。

**记录**：本规则目前在 fixture 装载边界生效（不改公共领域模型）；B2 实现各平台
真实采集器时，把原始数据转换为 `SocketEntry` 的代码路径必须复用同一规则——届时
应把该校验函数提升到 `runquiry-core` 公共模块（或 platform 共用模块），而非重写
一份。三种损坏层（结构截断 → 解析失败；`port: 0` → 反序列化失败；协议-端口配对
错误 → 规则失败）分别由 macos / linux / windows 的 `sockets-malformed.json` 覆盖。

## 5. 敏感信息扫描结果

扫描命令与输出（2026-09-03，30 个 JSON 文件全部无命中）：

```console
$ grep -rniE 'syspetro|/home/[a-z]' tests/fixtures/ ; echo "exit=$?"
exit=1        # 无命中（grep 退出码 1 = 未找到）
$ grep -rniE 'token|secret|password|api[_-]?key' tests/fixtures/ ; echo "exit=$?"
exit=1        # 无命中
$ find tests/fixtures -name '*.json' | wc -l
30
```

补充约定：fixture 中不含真实进程数据、真实容器 ID、真实镜像仓库之外的
网络地址（`registry.example.internal` 为保留示例域）；环境变量字段
（`ProcessDetails.environment`）在 fixture 中保持为空，避免任何真实变量值。

## 6. 验证记录

- `cargo test -p runquiry-core --locked`：36 通过（原 12：domain_types 11 + ports 1；
  另有 B4 的 core 侧在 `src/refresh.rs` 增加的 6 个 lib 单元测试；本次新增
  `fixtures_load` 14 + `process_controller_contract` 4）。
- `cargo test -p runquiry-platform --locked`：10 通过（全部为本次新增）。
- `cargo clippy` / `cargo fmt --check`：本次改动文件格式化后零 diff；
  clippy 结果依赖 `src/refresh.rs`（并行任务）修复后复跑全量。