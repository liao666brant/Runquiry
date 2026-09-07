# Runquiry Batch 4A I1 独立复审

日期：2026-09-04
范围：当前未提交 worktree；重点复验 I1 初审的五项阻断与 B5/B6 装配。
审查方式：只读代码审查 + 本次独立运行的 locked 门禁。未依赖之前的 DoneClaim。

## 结论

- `codeQualityStatus`: WATCH
- `recommendation`: APPROVE
- `blockers`: 无

五项原阻断均已关闭：

1. `CompactDetail` 持有 `Entity<AppShell>`，在 `new` 中 `observe` 壳层并 `notify`，每次 render 从壳层重新取得 `detail_content`（`crates/runquiry-ui/src/shell/compact_detail.rs:7-29`）；异步详情成功路径会 `cx.notify()`（`shell/interactions.rs:66-80`）。不再是打开 Sheet 时复制的静态摘要。
2. 容器解析在没有可验证的 host PID 或 PID 不在进程快照时生成 `InvestigationTarget::Container`，而非误报 NotFound（`crates/runquiry-app/src/backend/container.rs:65-78`）；该分支有行为测试（同文件:88-110）。
3. 三个 B6 详情均用选择的稳定 key 从最新快照再定位，并将 `issues` 随详情带入（`crates/runquiry-ui/src/workspaces/detail.rs:31-82`）；Ports、Containers 与 File Locks 分别展示其完整行字段和 partial issue 面板（`shell/render_workspace_detail.rs:16-121`）。
4. UI 真实事件路径在筛选、模式、选择、排序前调用 `invalidate_active_refresh`，并推进 `AppSession` generation（`shell/workspace_interactions.rs:13-135`）；后台结果同时核对 workspace、generation 与当前 active gate（`shell/refresh.rs:28-52`）。测试用真实 `AppSession` 交互逐项验证 filter/selection/mode/sort 使 gate 失效（`src/backend.rs:210-271`）。
5. `ProcessCommand::bindings()` 由 app 的实际 `cx.bind_keys` 消费并映射到四类 GPUI action（`crates/runquiry-app/src/main.rs:56-67`），不再是未引用的常量契约。

边界检查：UI 只依赖 core；`runquiry-platform` 仅在 app 装配层导入（`crates/runquiry-app/src/backend.rs:13`）。所有同步 `WorkspaceBackend` 调用由 `background_spawn` 发起（`crates/runquiry-ui/src/shell/refresh.rs:20-25`、`shell/interactions.rs:59-81,105-112,171-197`）。默认脱敏、显式 reveal 和引号敏感参数回归测试见 `processes/redaction.rs:89-140` 与 `processes/tests.rs:174-205`。窗口入口确以 `Root` 为第一级视图（`runquiry-app/src/main.rs:97-113`）；窄窗 Sheet 由 `open_sheet` 承载实体视图（`shell/interactions.rs:205-215`）。

## 验证

本次运行且通过：

```text
cargo test -p runquiry-core --locked       # 107 passed
cargo test -p runquiry-platform --locked   # 97 passed
cargo test -p runquiry-ui --locked         # 47 passed
cargo test -p runquiry-app --locked        # 15 passed
cargo check -p runquiry-app --locked
cargo fmt --check
git diff --check
```

新增/重写生产 Rust 文件按非空、非注释行计数均未超过 250；最高为 `crates/runquiry-ui/src/shell/render_detail.rs` 的 228 行。对 UI 与 app 的本轮生产代码扫描未发现 `unwrap(`、`expect(`、`panic!`、`todo!` 或 `unimplemented!`。没有根 Cargo 配置、Cargo.lock、deny、toolchain 或 `witr/` diff。

## Findings

### CRITICAL

无。

### HIGH

无。

### MEDIUM

1. `ProcessCommand` 的单元测试仅断言自身返回的常量数组（`crates/runquiry-ui/src/processes/tests.rs:280-286`），没有从 app 装配层验证该数组实际变成 `KeyBinding`。这是 implementation-mirroring / 弱价值测试；生产消费已由代码审查确认，不影响本次批准。若后续建立 app action 测试面，应替换为端到端键盘行为或装配映射测试。

### LOW

1. 对全部警告使用 `-D warnings` 的严格命令无法在当前基线通过：先在锁定 GPUI 依赖图触发 77 个 `clippy::multiple_crate_versions`，对该 lint 放行后仍因既有 `eprintln!`（`crates/runquiry-app/src/main.rs:126,185`）触发 `clippy::print_stderr`。根 `Cargo.toml` 明确将 `print_stderr` 配置为 warn，因此这不是 I1 变更回归；常规 workspace lint 和本轮 crate check/test 均为绿。后续若团队把警告升级为错误，应先决定统一日志策略及第三方多版本依赖例外。

## Skill-perspective check

已完整阅读并应用 `remove-ai-slops` 与 `programming` 的审查视角。`remove-ai-slops` 通过：未见生产层无必要解析/归一化、死代码、过大模块或为删除而写的测试；但指出上述 `ProcessCommand` 常量测试偏自证。`programming` 通过：未见 untyped escape hatch、裸 `unwrap/expect/panic`、UI 到 platform 的逆向依赖或同步平台调用进入 GPUI 前台；`WorkspaceBackend` 是 app/UI 的真实测试与线程边界，非无效抽象。

## Residual risks

本报告仅覆盖静态、构建和单元测试。容器 runtime 可用性、Sheet 焦点恢复、真实输入/表格交互与宽窄窗视觉表现仍应由后续独立 Linux GPUI 手工 QA 验收。
