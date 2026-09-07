# Batch 4A B5 Processes 证据

日期：2026-09-04

## 范围

- 仅修改 `crates/runquiry-ui/src/processes/**`。
- 未修改 app、shell/session、core/platform、依赖或锁文件。
- 真实窗口装配与平台采集由后续集成任务完成；本任务交付可组合的纯状态、
  `TableDelegate`/`DataTable` 组件 seam 与测试。

## TDD 证据

### 绿基线

- 场景：新增 B5 测试前确认 UI crate 既有行为正常。
- 调用：`cargo test -p runquiry-ui --locked`
- 可观察结果：退出 0；21 passed、0 failed。

### Red

- 场景：先声明 B5 行为测试与模块边界，不提供实现。
- 调用：`cargo test -p runquiry-ui --locked`
- 可观察结果：编译在 `processes/{detail,model,query,redaction,table}.rs` 尚不存在处以
  `E0583` 失败，证明测试先于生产实现加入。
- 原始日志：`/tmp/runquiry-b5-red.log`。

### Green

- 场景：五种显式目标、零/唯一/歧义、稳定完整身份、PID 复用/消失、
  identity+generation stale、CPU 采样中、部分成功/能力边界、详情分区、
  会话级脱敏、键盘事件与 100k 行共享模型。
- 调用：`cargo test -p runquiry-ui --locked`
- 可观察结果：退出 0；42 passed、0 failed，其中 9 项为 `processes::tests`；
  最终联合运行无编译警告。

### 独立审查安全回归

- 场景：命令行为 `server --token "top secret" --mode safe`；未 reveal 时不得泄露
  被 `split_whitespace` 拆开的 `secret"` 尾段。
- Red 调用：
  `cargo test -p runquiry-ui --locked processes::tests::quoted_sensitive_argument_never_leaks_trailing_value_fragments`
- Red 可观察结果：退出 101；断言收到
  `["server", "--token", "••••••••", "secret\"", ...]`，确认真实泄露。
- Green 可观察结果：退出 0；输出仅为
  `["server", "--token", "••••••••"]`，敏感值之后的载荷被保守截断。
- 原始日志：`/tmp/runquiry-b5-review-red.log`、`/tmp/runquiry-b5-review-green.log`。

## 100k 行真实模型调用

- 场景：真实构造 100,000 个 `ProcessSummary`，验证 `Arc` 同一底层存储、
  可见区只借用 32 行切片，并验证同一完整领域身份在排序位置改变后生成相同的行 ID。
- 调用：
  `cargo test -p runquiry-ui --locked processes::tests::large_process_model_shares_storage_and_keeps_domain_row_id_stable`
- 可观察结果：退出 0；1 passed、0 failed；用时 0.04s。
- 原始日志：`/tmp/runquiry-b5-100k.log`。
- 证据边界：此测试不证明 GPUI 实际可见行数量或窗口渲染性能；`DataTable` 的可见范围
  回调和真实虚拟渲染必须由后续集成窗口 QA 观察。

## 编译与格式验证

- `rustfmt --edition 2024 crates/runquiry-ui/src/processes/*.rs`：退出 0。
- `cargo check -p runquiry-ui --locked`：退出 0。
- `cargo check -p runquiry-app --locked`：退出 0。
- `cargo clippy -p runquiry-ui --locked --all-targets -- -D warnings -A clippy::multiple-crate-versions`：
  退出 0；`-A` 仅屏蔽既有依赖图的多版本报告。B5 生产代码的 `doc_markdown`、
  `match_same_arms`，以及测试代码的 7 个 `expect_used` 与 1 个
  `redundant_clone` 均已修复；没有新增 `allow` 或降低 lint。
- `git diff --check`：退出 0。
- 生产文件纯代码行数：`command.rs` 29、`detail.rs` 54、`mod.rs` 16、
  `model.rs` 133、`query.rs` 59、`redaction.rs` 124、`surface.rs` 107、
  `table.rs` 181；全部低于 250。

## 可组合 seam

- `TargetKind`/`QueryOutcome`：入口类型由用户显式选择，歧义不默认首项。
- `ProcessesState`/`DetailRequest`：列表刷新保存完整 `ProcessIdentity`，详情结果
  同时校验 identity 与 generation。
- `SurfaceSnapshot`：部分成功时同时借用原始数据与结构化诊断，能力边界不伪装空态。
- `DetailPrivacySession`：环境和命令参数默认脱敏，选择、刷新、身份变化时清除 reveal。
- `AnalysisSections`：仅提供概览、祖先、来源、告警、资源、Socket、文件、环境八区的
  结构化摘要指标 seam；不是八区详情 UI，具体渲染与 partial banner 仍由集成任务完成。
- `ProcessTableDelegate`/`ProcessTable`：使用 pinned gpui-component `TableState`/
  `DataTable`，领域行 ID 不依赖行 index；真实虚拟滚动由后续窗口 QA 验证。
- `ProcessCommand`：为后续 shell 绑定 Ctrl/Cmd+K、Ctrl/Cmd+1..4、Enter、四方向与
  Escape 提供纯事件契约。

## 对抗项与剩余风险

- 已覆盖 PID 同值但 start time 改变、选择目标消失、刷新后旧请求返回、无 CPU 首样本、
  partial 数据伴随权限诊断、Unsupported/Unavailable/PermissionDenied 分离、敏感参数的
  `--key=value`、`--key value` 与带空格引号值、100k 行模型。
- 未在本任务运行真实窗口 QA：页面尚未由共享 shell/app 装配，也尚未注入真实平台采集器；
  该验证必须在后续集成门通过 `DISPLAY=:0` 的 WSLg 产品窗口完成。
