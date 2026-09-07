# Batch 4A I1 跨层集成独立代码审查

## 审查范围

- 基线：当前 `HEAD`（`e0bf661`）的未提交工作区；审查窗口为 `runquiry-ui` 后端、`shell/**`、i18n、App 后端/装配，以及为接线改动的页面适配。
- 需求来源：[模块 05 计划](../plans/runquiry-gpui-desktop/05-desktop-ui.md)，尤其 B5、B6 和「布局与交互验收」；[witr parity](../../docs/witr-parity.md) §2、§12；[DESIGN.md](../../DESIGN.md) §5、§7、§8。
- 规范来源：根 `AGENTS.md`（分层、无 UI 系统调用、最小验证），`DESIGN.md`。
- 审阅证据：实现者 [shell 集成证据](batch4a-shell-integration.md) 及源码。截图文件均存在于其列出的 `/tmp` 路径；本审查只核验其存在与时间，不作视觉美学结论。

## Standards

### CRITICAL

无。

### HIGH

1. `crates/runquiry-ui/src/shell/interactions.rs:203-213`：窄窗 Sheet 只捕获选择摘要，不能显示异步完成的真实详情。
   - 证据：`open_compact_detail` 在选择时把 `compact_detail_summary()` 的 `String` clone 进 Sheet；进程分析在 `on_process_table` 的后台任务中才于 `:43-56` 写入 `shell.analysis`。详情渲染只发生在 `render_detail_area` / `detail_content`（`render_detail.rs:20-71`），而窄布局不会渲染该区域（`render.rs:79-81`）。Sheet 因而既不绑定 shell entity，也不会随 `cx.notify()` 重绘。
   - 影响：960–1099px 下，B5 要求的概览、祖先、来源、告警、资源、Socket、文件和环境详情不可达；B6 的详情同样退化成一行摘要。这违反模块计划的窄窗 Sheet 和 B5/B6 详情验收，也与 DESIGN.md §7/§8 的可达详情路径不符。
   - 建议：让 Sheet 的内容绑定/渲染 `AppShell::detail_content`（或单独的、可更新的详情 entity），并以真实 UI 回归证明分析完成、候选切换及 Escape 后焦点恢复都在 Sheet 内正确更新。

2. `crates/runquiry-app/src/backend.rs:144-169`：唯一命中的容器若没有已验证宿主 PID，被错误地当成「未找到」而不是容器 fallback。
   - 证据：容器解析得到唯一 `ContainerKey` 后，只有 `verified_host_pid` 返回 `Some` 才形成 PID；空集合在 `:161-166` 直接返回 `InspectError::NotFound`。`docs/witr-parity.md:105`、`:126` 明确要求主机 PID 缺失或无法验证时渲染容器自身的 fallback，而不是失败。
   - 影响：容器本身存在、但平台无法映射 host PID 的正常可降级情形无法通过五类调查入口完成调查，且与报告中「五类显式调查」的完成声明不符。
   - 建议：把容器 fallback 作为 UI 后端可表达的解析/详情结果，而非强制降为 `ProcessIdentity`；保留 host PID 成功时的完整进程分析，并为无 PID 的唯一容器补充端到端契约测试。

3. `crates/runquiry-ui/src/shell/render_detail.rs:180-208`：Ports、Containers、File Locks 的所谓详情只有格式化后的稳定键，并未呈现选中行的字段或诊断。
   - 证据：三分支分别仅返回 `address:port/protocol/PID`、`runtime/id`、`path/PID` 字符串。B6 要求三个页面均有「稳定选择、详情和 generation」，并要求 Partial 显示已获取数据及对应 issue；这里选中行的 state/owner/public-bind、container status/health/image/host PID、file lock type/mode/FD 均未出现在详情，diagnostic 也未出现。
   - 影响：B6 的详情验收不可达，且窄窗证据所称的「进入详情」并不等价于用户可检查真实对象详情。
   - 建议：以领域行/稳定键定位当前快照并渲染该行完整字段与相关 issue；页面切换或新快照使键过期时应明确显示 stale/不可用，而不是保留摘要。

### MEDIUM

1. `crates/runquiry-ui/src/shell/interactions.rs:153-174` 与 `crates/runquiry-ui/src/shell/data.rs:111-136`：页面交互没有接入 `AppSession` 的 generation/selection/filter/sort 状态机。
   - 证据：筛选、模式和表格选择只更新 `ShellData` 中的页面状态；没有调用 `WorkspaceSession::select`、`set_filter` 或 `set_sort`。但刷新回调只用 `WorkspaceResultGate` 对 `self.session.session(workspace).generation` 作校验（`refresh.rs:34-48`）。因此计划中已有的「选择/筛选/排序变化使在途刷新结果过期」契约未在真实壳层路径执行。
   - 影响：单元测试覆盖了孤立 session 状态机，却没有锁住产品接线；未来改变刷新/行映射时容易把旧快照应用到已改变的会话上下文，且持久会话状态与真实表状态会分叉。
   - 建议：选择一个唯一的会话状态所有者（现有 `AppSession` 或页面 state），把真实 UI 事件接到同一 generation gate，并补一个假后端的壳层集成测试：在 load 未完成时更改筛选/选择/排序，旧结果不得应用。

2. `crates/runquiry-ui/src/processes/command.rs:5-31`、`crates/runquiry-ui/src/processes/tests.rs:277-304`：`ProcessCommand` 仅由同一自证式单元测试使用，未参与 GPUI action/keybinding 接线。
   - 证据：全仓对 `ProcessCommand` 的引用仅为 re-export 和测试；实际键绑定是 `main.rs:55-70` 与 `shell/render.rs:53-69` 的 action handlers。这一 parser 对真实按键行为没有约束。
   - 影响：测试提供了键盘契约已验证的假信号，并留下无消费的生产抽象；未来真实 binding 失效时它仍会通过。
   - 建议：删除未使用的 `ProcessCommand` 及其映射测试，或把测试改为覆盖实际 action/keybinding/表格事件路径。此项是 remove-ai-slops 所定义的无用测试与 needless abstraction。

### LOW

无。

## Spec

- [P1] 窄窗详情仅摘要：同 Standards HIGH-1；违反模块 05 的「更窄时详情使用 Sheet」和 B5 详情覆盖要求。
- [P1] 容器无 host PID 没有 fallback：同 Standards HIGH-2；违反 `docs/witr-parity.md:105,126`。
- [P1] B6 三页没有可用详情：同 Standards HIGH-3；违反模块 05 B6「详情和 generation」与 Partial 呈现要求。
- [P2] 真实接线未使用既有会话 generation 状态：同 Standards MEDIUM-1；模块 05 要求 generation，当前仅隔离单元模型被验证。

## 已核验项

- UI crate 没有平台/OS 直接调用；`LinuxPlatform` 与 `ContainerRuntimes` 仅在 `runquiry-app::backend` 装配。同步 load/resolve/analyze 都经 `background_spawn`，而 UI 回调回前台实体。
- App 真实构造并共享 `Arc<LinuxPlatform>` 与 `Arc<ContainerRuntimes>`；`Root` 保持窗口第一级视图。无根 Cargo、锁文件或 `witr/` 改动。
- 四工作区均有 `DataTable`/`TableState` 接线；delegate 重写稳定 row ID；未发现 UI crate 的禁用宏、`unwrap`/`expect`/`panic`、或超过 250 纯 LOC 的生产文件。
- `cargo test -p runquiry-core --locked`：107 passed；platform：97 passed；UI：43 passed；app：14 passed。
- `cargo check -p runquiry-app --locked` 与严格 clippy（仅调用方批准的两个例外）均 exit 0；`cargo fmt --check`、`git diff --check HEAD` 均 exit 0。

## 技能视角

已加载并执行 `remove-ai-slops` 与 `programming` 的审查视角。发现 `ProcessCommand` 及其测试为未接入生产行为的 needless abstraction / implementation-mirroring test（MEDIUM）。未发现 untyped escape hatch、无依据的数据解析、额外依赖或 UI 层平台调用。除上述问题外，生产模块规模符合本轮 `<250` 纯 LOC 要求。

## 结论

- `codeQualityStatus`: BLOCK
- `recommendation`: REQUEST_CHANGES
- `blockers`：修复窄窗 Sheet 的真实详情渲染；实现唯一容器无 host PID 的 fallback；为 B6 三页提供真实、带 partial/stale 语义的详情。
