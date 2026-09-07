# Batch 4A 显式 Port/File 目标 backend 复现与修复

日期：2026-09-04

## 结论

`PlatformBackend` 不再跨操作持有 `LinuxPlatform`。每次 `load`、`resolve` 与
`analyze` 创建一个 fresh `LinuxPlatform`，并只在该次操作内复用同一实例；容器
运行时仍由 `Arc<ContainerRuntimes>` 共享。这样默认 Processes 清单继续排除
Runquiry 自身和本轮辅助后代，而下一次用户刷新/显式调查会包含应用启动后出现的
外部进程。

`Inspection.data = None` 在 Port/File/进程清单 resolve 边界不再被转换为空集合或
“未找到”：端口/文件/清单不可用会返回 `InspectError::Unsupported`，端口已存在但
容器发布端口 fallback 无数据时保留 `SocketOwnerUnknown`。

## 根因和 toggle proof

Linux 平台的进程清单以构造时基线及 `constructed_at` 排除自身和构造后进程。旧
长寿命 backend 可从网络/文件采集拿到 owner PID，却在同一旧平台的进程清单中找
不到该 PID，最终由 `map_pids` 返回 `NotFound { subject: "仍存活的进程候选" }`。

受控测试先构造 backend/旧平台，再启动独立测试子进程。子进程分别持有
`127.0.0.1:0` TCP listener 或唯一 `/tmp/runquiry-backend-target-<pid>-<nanos>` 文件。
测试先断言 fresh 网络/文件采集含子进程 PID，再断言构造前的旧进程清单不含该 PID。

| 阶段 | 同一场景调用 | 二进制可观察结果 | 原始产物 |
|---|---|---|---|
| 旧实现 | `cargo test -p runquiry-app --locked backend::tests::resolves_post_start_loopback_port_holder -- --exact --test-threads=1` | RED：`NotFound { subject: "仍存活的进程候选" }` | `/tmp/runquiry-batch4a-target-backend-old-red.log` |
| fresh 实现 | 同上 | GREEN：1 passed | `/tmp/runquiry-batch4a-target-backend-fresh-green.log` |
| 再次临时恢复旧实现 | 同上 | RED：同一 `NotFound` | `/tmp/runquiry-batch4a-target-backend-restored-old-red.log` |
| 最终还原 fresh | `cargo test -p runquiry-app --locked -- --test-threads=1` | GREEN：19 passed（含 Port/File 两个真实回归） | 本文件的最终门禁记录 |

临时旧实现只用于 toggle proof，已还原；最终 backend 源哈希为
`33bf6ffa3d8fb6f0d8c5e5db790a31d222a48c86f928deeffa958f239d399363`。

## 最终门禁

| 成功条件 | 调用 | 结果 | 可复核产物 |
|---|---|---|---|
| 真实 Port/File 解析和 app 回归 | `cargo test -p runquiry-app --locked -- --test-threads=1` | 19 passed，0 failed；独立子进程 owner PID 被解析为 `InvestigationTarget::Process` | 本文件；`crates/runquiry-app/src/backend/tests.rs` |
| 编译 | `cargo check -p runquiry-app --locked` | exit 0 | 本文件 |
| 严格 lint | `cargo clippy -p runquiry-app --all-targets --locked -- -D warnings -A clippy::multiple_crate_versions -A clippy::print_stderr` | exit 0；仅仓库既定两项例外 | 本文件 |
| 格式与差异空白 | `cargo fmt --all -- --check`；`git diff --check HEAD`；三个 backend untracked 文件的 `git diff --no-index --check` | 全部 exit 0 | 本文件 |
| 生产文件规模 | `awk '!/^[[:space:]]*$/ && !/^[[:space:]]*(//|#)/'` | `backend.rs` 249 pure LOC；`backend/container.rs` 99 pure LOC | 本文件 |
| 清理 | 完整 app 测试结束后检查 `/tmp/runquiry-backend-target-*` 和 helper 测试进程 | 无匹配文件、无 helper 进程 | 本文件 |

检查时发现 `127.0.0.1:45678` 与 `127.0.0.1:46245` 仍在监听；它们不是本测试从
内核分配的临时端口，未触碰。

## 独立复审修复：fresh 平台的跨分析 single-flight

复审指出：fresh `LinuxPlatform` 为每次分析创建独立的 `systemd_in_flight`，会使
两个 UI background analysis 同时进入 systemd D-Bus 富化。已保留每次
`load`/`resolve`/`analyze` 的 fresh 平台规则，并新增共享的
`backend/analysis_gate.rs`：`PlatformBackend` 跨操作持有 `AnalysisGate(Mutex<()>)`，
仅包围完整 `analyze`。锁中毒映射为 `InspectError::Unsupported`，不会 panic 或静默
继续；`MutexGuard` 的 RAII 在分析返回时释放门控。

`backend::tests::serializes_concurrent_analysis_admission` 将同一个
`Arc<PlatformBackend>` 交给两个线程，经生产 `with_analysis_gate` seam 同时进入，
并用同步 barrier、每个 worker 独立 release channel 和原子最大并发计数观测准入。
该测试不依赖真实 systemd 或时间序列：禁用 Mutex 后两个工作项都进入闭包且
`maximum = 2`；恢复 Mutex 后第二项只能在第一项释放后进入且 `maximum = 1`。

| 阶段 | 调用 | 可观察结果 | 原始产物 |
|---|---|---|---|
| TDD RED | `cargo test -p runquiry-app --locked backend::tests::serializes_concurrent_analysis_admission -- --exact --test-threads=1` | 缺少 production sharing seam，编译报 `with_analysis_gate` 不存在 | `/tmp/runquiry-batch4a-analysis-gate-red.log` |
| 门控禁用 RED | 同上，临时让 `AnalysisGate::run` 直接执行 closure | 断言失败：`left: 2`，`right: 1` | `/tmp/runquiry-batch4a-analysis-gate-disabled-red.log` |
| 最终 GREEN | `cargo test -p runquiry-app --locked -- --test-threads=1` | 20 passed，包含 single-flight 与 Port/File 真实资源回归 | 本文件的最终门禁记录 |

UI 调查详情仍使用 `cx.background_spawn` 调用 backend analyze；旧/新任务只在后台等待
门控，generation gate 继续在 UI 侧丢弃过期结果，未增加前台阻塞。

最终复审修复门禁：

- `cargo test -p runquiry-app --locked -- --test-threads=1`：20 passed。
- `cargo check -p runquiry-app --locked`：exit 0。
- `cargo clippy -p runquiry-core -p runquiry-platform -p runquiry-ui -p runquiry-app --all-targets --locked -- -D warnings -A clippy::multiple_crate_versions -A clippy::print_stderr`：exit 0。
- `cargo fmt --all -- --check`、`git diff --check HEAD` 及五个 backend 文件的 no-index diff check：exit 0。
- 生产 pure LOC：`backend.rs` 216、`backend/analysis_gate.rs` 23、`backend/unavailable.rs` 49、`backend/container.rs` 99。
- 清理检查没有 `runquiry-backend-target-*` 文件或 helper 测试进程；仅保留非本测试的 `127.0.0.1:46245` 监听，未触碰。
