# Batch 4A B5 Processes 独立代码审查

审查范围：`crates/runquiry-ui/src/processes/**`，并核对其在现有 shell/app 中的实际接入；基线
提交为 `e0bf6617d0541e8aae0df644699982c0f073bb02`。并行进行的 P1/B6 改动没有归因给 B5，
但会影响联合验证结果。

## 结论

- `codeQualityStatus`: BLOCK
- `recommendation`: REQUEST_CHANGES

## CRITICAL

无。

## HIGH

1. B5 尚未交付可使用的 Processes 工作区，而只是未被消费的 seam。`ProcessTable`、目标类型、
   候选状态、详情模型和键盘枚举只在新模块及其单元测试中出现；没有任何 shell/app 调用点建立
   `TableState` 或渲染 `ProcessTable`。现有壳层仍无条件渲染 Unsupported 状态和空详情
   （`crates/runquiry-ui/src/shell/render.rs:180-216`），`refresh_active`/`poll_detail` 也明确
   保持占位（`crates/runquiry-ui/src/shell/mod.rs:205-237`）。因此五类调查、候选选择、详情八区、
   500ms 详情加载、窄窗 Sheet、真实采集和键盘流程均不能由用户触发。证据文件也承认“后续集成
   任务”（`.omo/evidence/batch4a-b5-processes.md:7-10,74-75`），这与 B5 计划要求的“实现…工作区”
   不符。必须在共享 shell/app 完成真实装配后，才可将 B5 标为完成。

2. 默认脱敏会泄露带空格/引号的敏感命令参数。`DetailPrivacySession::command_line` 使用
   `split_whitespace()`（`crates/runquiry-ui/src/processes/redaction.rs:105-127`）；例如
   `--token "top secret"` 仅遮罩第一个值片段，后一个 `secret"` 会作为普通参数展示。B5 的
   脱敏要求覆盖命令参数，不能把 shell 词法边界拆坏后泄露其余内容。现有测试只验证单词值
   `--token hunter2`（`crates/runquiry-ui/src/processes/tests.rs:149-166`）。应使用与输入契约一致的
   tokenization，或在无法安全解析时保守遮罩整个后续参数载荷，并新增含空白/引号的回归测试。

3. 所要求的 `cargo test -p runquiry-ui --locked` 当前不能编译，故 B5 的联合测试绿灯证据已失效。
   本次复跑在 `crates/runquiry-ui/src/workspaces/tests/common.rs:14,57,69,74` 报出传值而非引用的
   `E0308`。这些行属并行 B6 变更、不是 B5 责任，但在修复前不能声称当前 UI crate 的 B5 测试
   套件通过；后续 B5 集成应在 B6 修复后重新运行并记录结果。

## MEDIUM

1. “100k 行真实虚拟化”测试只证明手写 `ProcessRows` 共享 `Arc`，及一个恒为零的
   `rendered_rows()` 方法（`crates/runquiry-ui/src/processes/model.rs:78-82`；
   `crates/runquiry-ui/src/processes/tests.rs:216-225`）。它没有创建 `TableState`、没有触发
   `visible_rows_changed`，也没有证明 DataTable 未全量构造元素；当前还没有任何生产调用点创建
   此表。按 remove-ai-slops 视角，这是一项以实现常量代替可观察虚拟化行为的弱测试。应在真实
   GPUI 表格装配后以可见范围回调/实际窗口场景验证，删除或替换 `rendered_rows()` 断言。

2. `AnalysisSections` 仅从 `Analysis` 计算八个计数（`detail.rs:33-63`），不暴露或渲染概览、来源
   证据、告警、资源、Socket、文件、环境的实际字段；`issue_count` 同样丢失具体诊断。它没有
   假造字符串，方向正确，但不能满足“详情覆盖八区”或 partial 问题可解释的用户行为。集成时应
   直接消费 core 的结构化字段和 `DiagnosticIssue`，不以该计数模型充当详情完成证据。

3. `SurfaceState::Partial` 只保存 issue 数（`surface.rs:17-20`），而表格在有数据时没有任何诊断
   呈现路径；无数据时还把 Partial 映射为 Empty（`table.rs:104-121`）。这会让部分结果的受限
   原因在实际表面不可见，违背状态矩阵。应在集成层渲染保留的 `SurfaceSnapshot::issues()`，并为
   有数据和无数据两种 partial 情形加用户可见测试。

## LOW

1. B5 新生产文件本身的 clippy 还存在三项可修复错误：`DataTable` 未用反引号的 doc markdown
   （`model.rs:56`, `table.rs:21,69`），以及 `SurfaceSnapshot::new` 的重复 match arms
   （`surface.rs:78-88`）。`cargo clippy -p runquiry-ui --locked --all-targets -- -D warnings
   -A clippy::multiple_crate_versions` 因这些错误（以及并行 B6 错误）失败。

## 已核验项

- 五种 `TargetKind` 的 parse 路径确实显式映射到 core `QueryTarget`，没有以输入内容猜类型
  （`query.rs:28-41`）；`QueryOutcome::selected` 对 Ambiguous 返回 `None`（`query.rs:81-86`）。
- `ProcessesState` 用完整 `ProcessIdentity` 保存选择，刷新时区分保留/PID 重用/消失，并在
  `accepts` 同时检查 identity 与 generation（`model.rs:124-188`）。这只是正确的可集成 seam；
  尚未接入异步详情流程。
- 表格行容器 ID 由 PID + start time 派生而不是 row index（`table.rs:74-81,124-136`）；使用了真实
  `TableDelegate`/`DataTable` API。行 ID 对缺失启动时间只能是展示稳定键，不能替代 core 的
  `same_process` 判定，后者在任一 start time 缺失时保守为 false。
- 新生产文件纯代码行均低于 250，未发现生产路径中的 `unwrap!`、`expect!`、`panic!`、`todo!`、
  `unimplemented!`、`unsafe` 或新增依赖。`cargo check -p runquiry-app --locked` 通过；
  `cargo fmt --check` 与 `git diff --check` 通过。
- 未运行真实窗口：当前尚无 Processes 视图装配，故无法验证真实 Linux 采集、脱敏显示、100k
  虚拟滚动、Sheet、焦点或快捷键。此项不能以现有 unit test/evidence 替代。

## 技能视角

已加载并应用 `remove-ai-slops` 与 `programming`。前者识别到 100k 测试将恒定内部标记当作
虚拟化证明；后者识别到字符串分词破坏敏感参数边界，且审查了类型/错误和生产文件尺寸。diff
没有无类型逃逸、新依赖、超大生产模块或不必要的数据解析层；但存在上述脱敏泄露、不可用交付
和不可运行联合测试，故不符合两个技能视角的可维护性与行为验证要求。

## 验证命令

- `cargo check -p runquiry-app --locked`：通过。
- `cargo fmt --check`：通过。
- `git diff --check`：通过。
- `cargo test -p runquiry-ui --locked`：失败，见 HIGH #3。
- `cargo clippy -p runquiry-ui --locked --all-targets -- -D warnings -A clippy::multiple_crate_versions`：失败，见 HIGH #3 与 LOW #1。

## 复审（2026-09-04，B5 自有修复）

### 结论

- `codeQualityStatus`: BLOCK
- `recommendation`: REQUEST_CHANGES
- B5 自有模块暂不具备 `PASS_FOR_INTEGRATION`：脱敏及虚拟化证据边界修复有效，但严格
  all-target clippy 仍由 B5 自有测试阻断。共享 shell/app 未接入仍是独立的集成阻断，不能当成
  该纯模块修复失败的归因。

### 已关闭的问题

1. 引号敏感值泄露已关闭。未 reveal 时，`command_line` 在敏感 assignment 或敏感 flag 后保守
   截断，而不是继续展示被空白拆开的余段（`redaction.rs:104-140`）。新增回归测试覆盖
   `server --token "top secret" --mode safe` 并断言没有 `top`/`secret` 片段
   （`tests.rs:168-181`）。此策略宁可不展示敏感值之后的非敏感参数，也不会泄露未知 shell
   quoting 的尾段；在未接入真实 UI 前这是合理的安全边界。

2. `rendered_rows()` 恒值断言已删除。100k 测试现在断言共享 `Arc`、按 index 取身份，并验证
   同一领域身份跨排序位置保持相同行 ID（`tests.rs:231-249`；`table.rs:64-86`）。证据文件也
   明确它不证明实际可见行数或窗口渲染性能（`batch4a-b5-processes.md` 的 100k 段）。这是可接受
   的 seam 测试，不再冒充真实虚拟化 QA。

3. `SurfaceSnapshot` 仍借用并公开完整 `DiagnosticIssue` 切片（`surface.rs:64-119`），不是只
   留计数；`AnalysisSections` 已在证据中准确降格为八区摘要 seam，而非详情 UI。真实 partial
   banner/八区字段展示仍由 shell 集成负责。

4. `cargo test -p runquiry-ui --locked` 本轮实跑通过：42 passed。`cargo check -p runquiry-app
   --locked`、`cargo fmt --check`、`git diff --check` 也通过；此前 B6 的 E0308 已不再出现。

### 仍需修复

1. `cargo clippy -p runquiry-ui --locked --all-targets -- -D warnings -A
   clippy::multiple_crate_versions` 本轮失败。B5 自有 `processes/tests.rs` 使用 7 处 `expect()`
   （`44,75,81-83,91,103-104,220`）并有一处冗余 clone（`60`）；项目 workspace 将
   `expect_used` 设为 deny，测试不能绕过该规则。应改成显式 `assert!(value.is_some())` +
   `unwrap_or...` 或模式匹配，并移除冗余 clone。B6 另有 3 个测试 clippy 失败
   （`workspaces/tests/file_locks.rs:54`、`workspaces/tests/ports.rs:80`），非 B5 归因。

2. 模块仍没有被 shell/app 创建或渲染：`shell/render.rs:180-216` 继续显示固定
   Unsupported/空详情，且当前没有 B5 符号的外部调用点。此项保留为 B5/B6 集成门阻断；在真实
   `TableState`、目标输入、候选、详情、Sheet 与采集器接线完成并做窗口 QA 前，不得宣布 B5
   用户功能完成。

## 二次复核（2026-09-04，B5 clippy 修复）

### 结论

`PASS_FOR_INTEGRATION`。B5 自有 `processes/**` 的 clippy/test-quality 问题均已关闭；I1 刚开始
写入的共享 `backend.rs` 目前不完整，阻断全 crate 命令，但不归因给 B5，也不改变本模块可供
集成的判定。

### B5 自有核验

- 原先的 `expect()` 均改为先断言再 `let ... else` 的显式失败路径，冗余 clone 已移除
  （`processes/tests.rs:33-255`）。未发现 B5 tests 或生产代码中的 `expect!`/`unwrap!`/`panic!`/
  `todo!`/`unimplemented!`。
- quoted-sensitive 回归与保守截断仍在，测试断言针对用户可见泄露片段，而非内部实现常量
  （`processes/tests.rs:190-203`）。
- 100k 测试不再断言固定 `rendered_rows`，只检查共享存储与排序无关领域行 ID
  （`processes/tests.rs:257-275`）；它明确不替代后续真实 DataTable 窗口 QA。
- 文件纯代码行均低于 250；当前 `git diff --check` 与 `cargo fmt --check` 通过。

### 当前命令状态与归因

- 本次运行 `cargo clippy -p runquiry-ui --locked --all-targets -- -D warnings -A
  clippy::multiple_crate_versions` 以及 `cargo test -p runquiry-ui --locked` 均在 I1 的新增
  `crates/runquiry-ui/src/backend.rs:7` 停止：测试模块导入 `super::WorkspaceResultGate`，但该类型
  尚未定义（`E0432`）。此前 B5/B6 test-lint 均未进入执行阶段，因此这不是 B5 失败的绿灯证据。
- I1 也刚将 `pub mod backend;` 加入 `crates/runquiry-ui/src/lib.rs`；目前没有 B5 符号的共享接入
  调用点。它属于 I1 整体可达性/装配工作，后续须由 I1 负责人修复并重新跑整套 clippy/tests。
