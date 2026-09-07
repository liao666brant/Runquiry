# Batch 4A / B6 Ports、Containers、File Locks 证据

日期：2026-09-04

## 变更边界

- 生产代码仅位于 `crates/runquiry-ui/src/workspaces/**`。
- 未修改 app、shell/session/state/locales、core/platform、根 Cargo/Cargo.lock 或 `witr/`。
- 未新增依赖，未提交或推送。

## 行为交付

- Ports：仅监听/全部 Socket、文本筛选、七列排序、无主 PID 保留、公开绑定标识、稳定复合键、stale selection。
- Containers：以 `ContainerKey` 稳定选择，区分已验证 host PID 与 fallback，partial 运行时数据不丢失。
- File Locks：仅锁/全部打开文件、FD、真实锁元数据、同 PID/路径锁优先、Windows Unsupported 与权限边界。
- 通用：`CapabilityStatus + Inspection` 映射 Ready/Empty/Error/PermissionDenied/Unsupported；有数据的 partial 用诊断/能力横幅 seam；旧 generation 拒绝。
- 三页各自提供真实 `gpui_component::table::TableState` / `DataTable` 构造、更新、渲染 seam；delegate 的 `rows_count` 只报告行数，`render_td` 仅按可见索引读取不可变 `Arc<[T]>` 快照。

## Failing-first

### 绿基线

开始修改时执行：

```text
cargo test -p runquiry-ui --locked
```

当次基线被并行 B5 尚未创建的 `processes/{detail,model,query,redaction,table}.rs` 阻断，原始日志：
`/tmp/runquiry-b6-baseline.log`。可复核的改动前独立绿基线由模块脚手架任务记录在
`.omo/evidence/batch4a-ui-module-boundary-scaffold.md`（`cargo check -p runquiry-ui --locked` exit 0）。

### 红测

在写入 B6 行为测试后执行：

```text
cargo test -p runquiry-ui --locked workspaces::tests
```

二进制可观察结果：exit 101；暴露 `StableSelection<K>` 错误 Default 约束、File Locks 首选行类型推断，以及 const `Option::map` 不可用。原始日志：`/tmp/runquiry-b6-red.log`。

### 绿测

最终定向执行：

```text
cargo test -p runquiry-ui --locked workspaces::tests
```

二进制可观察结果：exit 0，11 passed / 0 failed。原始日志：
`/tmp/runquiry-b6-green-final-targeted.log`。

## 验证矩阵

| 场景 | 调用 | 二进制可观察结果 | 原始产物 |
|---|---|---|---|
| UI 全包回归 | `cargo test -p runquiry-ui --locked` | exit 0；41 passed / 0 failed；doc tests 0 failed | `/tmp/runquiry-b6-ui-test-final.log` |
| UI 编译 | `cargo check -p runquiry-ui --locked` | exit 0 | `/tmp/runquiry-b6-ui-check-final.log` |
| app 消费编译 | `cargo check -p runquiry-app --locked` | exit 0 | `/tmp/runquiry-b6-app-check-final.log` |
| 自有文件格式 | `rustfmt --edition 2024 --check crates/runquiry-ui/src/workspaces/*.rs crates/runquiry-ui/src/workspaces/tests/*.rs` | exit 0 | `/tmp/runquiry-b6-rustfmt-final.log` |
| diff 空白 | `git diff --check` | exit 0 | `/tmp/runquiry-b6-diff-final.log` |
| 严格 Clippy（排除锁内已知多版本依赖及并行 B5 文档告警） | `cargo clippy -p runquiry-ui --lib --locked -- -D warnings -A clippy::multiple-crate-versions -A clippy::doc-markdown -A clippy::match-same-arms` | exit 0 | `/tmp/runquiry-b6-clippy-scoped-2.log` |
| 生产文件行数 | 对 `workspaces/*.rs` 统计非空、非纯注释行 | 最大 182，全部小于 250 | `/tmp/runquiry-b6-loc-final.log` |
| 禁止模式 | 扫描 unwrap/expect/panic/todo/unimplemented/unsafe/fs/Command | 生产文件无命中 | `/tmp/runquiry-b6-no-excuse-final.log` |

## 100k 手工状态驱动

调用：

```text
/usr/bin/time -f 'elapsed=%e max_rss_kb=%M' cargo test -p runquiry-ui --locked workspaces::tests::ports::hundred_thousand_rows_remain_index_backed -- --exact
```

二进制可观察结果：exit 0，1 passed；100,000 行可见索引建立后 `Arc::ptr_eq` 仍为真，证明没有复制领域行；耗时 0.50s，最大 RSS 157116 KiB（包含 Cargo/rustc 测试进程）。原始产物：`/tmp/runquiry-b6-100k-final.log`。

## 对抗项

- 无主 PID 不过滤、不编造进程名。
- `Inspection { data: Some, issues }` 保留数据并暴露 partial seam。
- Unsupported、PermissionDenied、Error、Empty 不互相伪装。
- 排序和刷新不以行索引保存选择；目标消失保留原 key 并标 stale，不自动跳到其他行。
- 筛选仅隐藏对象，不把隐藏误判为目标消失。
- 未验证 host PID 不进入进程详情，继续使用容器 fallback。
- 普通打开文件显示类型“打开”、模式“—”；真实锁不编造 FD。
- 同 PID/路径的普通 FD 与真实锁并存时，真实锁优先；普通 FD 重复时稳定选择最低 FD。

## 集成 seam 与剩余风险

- shell 持有三个 `Entity<TableState<...>>`，通过 `new_*_table` 构造、`update_*_table` 应用 generation 已验证的状态、`*_table_view` 渲染；`key_at` 把 `TableEvent::SelectRow(index)` 映射回领域 ID。
- `row(index)` 是详情面板 seam；页面模块不读取 OS、不运行命令。
- 真实窗口、工具栏输入/模式切换、宽窄详情 Sheet 与 Linux 平台数据由后续唯一 shell/app 集成负责人接线并执行视觉 QA；本模块自身没有独立可启动窗口，未伪造窗口验收。

## 独立复审后的 lint 修复

独立复审把验证面扩大到测试目标后，B6 测试暴露三项严格 lint：

- File Locks 测试为比较路径临时分配 `PathBuf`；改为借用 `Path::new`。
- 100k Ports 测试用 `as u16` 产生截断与符号丢失风险；改为有界 `u16::try_from`，不增加 allow。

修复前调用：

```text
cargo clippy -p runquiry-ui --locked --all-targets -- -D warnings -A clippy::multiple-crate-versions
```

二进制可观察结果：exit 101，共报告 B6 三项及当时并行 B5 八项；原始产物：
`/tmp/runquiry-b6-clippy-all-targets-before-fix.log`。

修复后以同一调用复验，二进制可观察结果：exit 0；原始产物：
`/tmp/runquiry-b6-clippy-all-targets-after-fix.log`。未新增 lint allow；唯一命令行 allow 是仓库锁定依赖树已知的 `multiple-crate-versions`。

最终回归：

| 场景 | 调用 | 二进制可观察结果 | 原始产物 |
|---|---|---|---|
| UI 全 targets Clippy | `cargo clippy -p runquiry-ui --locked --all-targets -- -D warnings -A clippy::multiple-crate-versions` | exit 0 | `/tmp/runquiry-b6-clippy-all-targets-after-fix.log` |
| UI 全包测试 | `cargo test -p runquiry-ui --locked` | exit 0；42 passed / 0 failed | `/tmp/runquiry-b6-ui-test-post-review.log` |
| 自有文件格式 | `rustfmt --edition 2024 --check crates/runquiry-ui/src/workspaces/*.rs crates/runquiry-ui/src/workspaces/tests/*.rs` | exit 0 | `/tmp/runquiry-b6-rustfmt-post-review.log` |
| diff 空白 | `git diff --check` | exit 0 | `/tmp/runquiry-b6-diff-post-review.log` |
