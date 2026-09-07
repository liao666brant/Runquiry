# P1 File Inventory 独立代码审查

审查范围：相对 `e0bf661` 的 P1 core/platform 改动；并行 UI 文件不计入本结论。

## 结论

- `codeQualityStatus`: BLOCK
- `recommendation`: REQUEST_CHANGES

## CRITICAL

无。

## HIGH

1. `FileInventory::list()` 会为每一个不可读 FD、每一个锁行的 `comm` 读取失败、以及每一个无法解析模式的锁行追加一条 `DiagnosticIssue`，没有聚合或上限。这违背 P1 的「100k 规模不产生逐行 issue」约束：`open_entries_for_pid` 在每个失败的 `read_link` 上 `push`（[open_files.rs](../../crates/runquiry-platform/src/linux/open_files.rs):66-69），`lock_entries` 对每条未知模式和每个 `comm` 失败分别 `push`（[locks.rs](../../crates/runquiry-platform/src/linux/locks.rs):103-108、124-126）。受限 `/proc` 或高 FD 数进程会分配和返回与行数同级的诊断集合，既拖慢列表，也会使 UI 诊断层失控。现有测试仅覆盖单个失败，未覆盖高基数聚合。应按稳定类别/进程聚合（计数和样本），并添加至少一个高基数失败测试，断言 issue 数有常数上界而数据仍保留。

2. `ProcessFileLocks::locks_of` 对锁持有进程的 FD 目录/readlink 失败保持静默：`fd_inode_map` 对 `read_dir_names` 与 `read_link` 均直接丢弃错误（[locks.rs](../../crates/runquiry-platform/src/linux/locks.rs):59-71），而 `locks_of` 只消费 `lock_entries` 的 issues（[locks.rs](../../crates/runquiry-platform/src/linux/locks.rs):144-154）。结果是锁路径退化为 `dev:inode` 时仍可返回 Complete，不能让调用方区分真实路径解析成功与局部权限/读取失败，违反 P1 的 partial/root-failure 诊断聚合语义。将 FD 解析失败纳入 `lock_entries` 的聚合诊断，并新增 `locks_of` 的受限 FD 断言。

## MEDIUM

1. 解析到 PID `0` 时，锁行会被重写为 `Pid::MIN`（即 PID 1）：[locks.rs](../../crates/runquiry-platform/src/linux/locks.rs):128-130。`Pid` 的领域不变量明确拒绝 0，当前回退把无效外部输入伪造成 systemd/init 的锁，可能导致错误调查结论。应跳过该行并记录聚合 `ParseFailed`，或将解析器的 PID 约束前置；补一条 PID 0 回归测试。

2. [file_inventory_contract.rs](../../crates/runquiry-core/tests/file_inventory_contract.rs):10-64 是自证式测试：Fake 实现原样返回测试刚构造的 `Vec`，断言同一 Vec 被原样取回。这只验证 `Inspection::complete` 和结构派生比较，不锁定 `FileInventory` 的可观察边界或 `fd`/`lock` 互斥语义。按 remove-ai-slops / programming 视角应删除或改为真实边界测试；现有 Linux 合成 `/proc` 测试已是正确方向，但需补充上述聚合与无效 PID 场景。

## LOW

无。

## 已核验项

- 公共契约以 `FileInventoryEntry { fd, lock }` 将普通 FD 与锁元数据分开；Linux 普通 FD 不再使用 `Other + Read` 伪造锁。`ProcessFileLocks` 仍返回原有 `FileLockEntry`。
- P1 没有修改 Cargo 依赖或锁文件；所审生产文件均低于 250 纯代码行：`file.rs` 42、`resolve.rs` 211、`locks.rs` 130、`open_files.rs` 137、`linux_qa.rs` 159。
- 未发现所审生产代码中的 `unwrap!`、`expect!`、`panic!`、`todo!` 或 `unsafe`；`unwrap_or` 不是宏，但 PID 0 回退见上述 MEDIUM。
- `cargo test -p runquiry-core --locked`：108 passed。
- `cargo test -p runquiry-platform --locked`：93 passed。
- `cargo clippy -p runquiry-core -p runquiry-platform --all-targets --locked -- -D warnings`：exit 0。
- P1 所有 Rust 文件的 `rustfmt --edition 2024 --check`：exit 0；`git diff --check`：exit 0。
- 审阅了 [P1 实现证据](batch4a-file-inventory-contract.md)。其真实 Linux QA 命令、受控进程、输出分类和清理回执可追溯；本审查未重新执行该受控运行期场景。该证据不足以覆盖上述高基数诊断和 `locks_of` 静默降级问题。

## 技能视角

已加载并应用 `remove-ai-slops` 与 `programming`：前者发现自证式契约测试，后者发现将非法 PID 0 映射为有效 PID 1 的类型不变量破坏。当前 diff 不存在无类型逃逸、新依赖或超大生产模块；但存在上述两个 HIGH 的可维护性/正确性问题，因此不能批准。

## 残余风险

即使修复上述问题，`holders()` 目前会先解析一次锁表、再通过 `inventory()` 解析一次完整锁表并扫描全体 PID（[open_files.rs](../../crates/runquiry-platform/src/linux/open_files.rs):127-145）。这不是本次阻断项，但高规模下存在重复扫描成本，应在修复后用基准或计数化 fixture 验证其是否满足交互刷新预算。

## 独立复审（P1 修复后）

### 结论

- `codeQualityStatus`: BLOCK
- `recommendation`: REQUEST_CHANGES

此前四项源码问题均已修复；当前唯一 blocker 是修复后真实 Linux QA 的成功输出与
清理回执没有可检查的原始产物路径。实现证据只给出了命令、摘要和
`cleanup=complete` 文本，未引用任何实际存在的日志/截图/结构化回执；工作区和
`/tmp` 检索均未找到该次 `runquiry-p1-review-qa` 的原始输出。因此不能把执行者的
陈述当作运行期验收。请把脱敏后的 QA 输出和清理结果保存为可追溯的 `.omo/evidence/`
产物（或提供实际的原始产物路径），再请求复审。

### 原阻断项逐项复核

1. **有界诊断：通过代码与测试反证。**
   [file_diagnostics.rs](../../crates/runquiry-platform/src/linux/file_diagnostics.rs):29-41
   定义固定 10 类；[into_issues](../../crates/runquiry-platform/src/linux/file_diagnostics.rs):125-138
   最多从这 10 个 bucket 各产生一条 issue。高基数测试
   [file_locks.rs](../../crates/runquiry-platform/tests/linux_adapters/file_locks.rs):4-45
   注入十万条非法锁模式和失败 FD，断言成功 FD 保留、issue 数受常数上界约束并报告
   聚合计数。实跑通过。
2. **`locks_of` FD 失败 Partial：通过。**
   [locks.rs](../../crates/runquiry-platform/src/linux/locks.rs):54-79 将 FD 目录/readlink
   错误记录到同一有界诊断器；[locks.rs](../../crates/runquiry-platform/src/linux/locks.rs):146-157
   把其转为 Partial。缺失目录和坏链接两个真实 adapter 测试均通过。
3. **PID 0：通过。**
   [locks.rs](../../crates/runquiry-platform/src/linux/locks.rs):103-107 在构造领域 `Pid`
   前拒绝 0 并记录 `ParseFailed`；对应真实 adapter 测试通过，未再映射为 PID 1。
4. **自证 core 测试：通过。** `crates/runquiry-core/tests/file_inventory_contract.rs` 已删除；
   公开契约由现有 core 消费者测试以及新增 Linux 合成 `/proc` adapter 行为测试覆盖。

### 本次独立验证

- `cargo test -p runquiry-core --locked`：107 passed。
- `cargo test -p runquiry-platform --locked`：97 passed，其中包括全部四项修复的 adapter
  行为测试。
- `cargo clippy -p runquiry-core -p runquiry-platform --all-targets --locked -- -D warnings`：exit 0。
- 所审 P1 Rust 文件 `rustfmt --edition 2024 --check`：exit 0；`git diff --check`：exit 0。
- `remove-ai-slops` / `programming` 视角复核：没有恢复自证测试、无新增依赖、无类型逃逸、
  无生产文件超过 250 纯代码行；`file_diagnostics.rs` 以一个稳定、受限职责的结构替代了
  逐行收集，未发现多余抽象。

## 真实 QA 证据补证复核

### 最终结论

- `codeQualityStatus`: CLEAR
- `recommendation`: APPROVE（PASS）
- `blockers`: 无。

[P1 实现证据](batch4a-file-inventory-contract.md) 的“修复复审后的真实 Linux QA 补证”
现在是可持久检查的自包含产物：它给出可重复的受控 `mktemp`/普通 FD/POSIX 锁命令、
与 `linux_qa` 示例入口一致的执行命令、`qa_exit=0`、关键分类输出，以及同一轮
`test ! -e` 和 `kill -0` 清理断言。输出未含真实 PID、用户名、随机目录后缀、全量路径
或环境变量值。该补证消除了上一轮唯一的证据 blocker；未发现 QA 进程、文件或目录遗留的
证据矛盾。

残余风险维持为非阻断项：`holders()` 的双重锁表/全 PID 扫描在极大规模下仍可能有刷新成本，
应由后续性能验收覆盖。

## Executor 修复回执（待独立复审）

本节记录对上述 REQUEST_CHANGES 的逐项处理，不修改原审查结论；最终状态由独立
守门复审决定。

1. HIGH 有界诊断：新增 `file_diagnostics.rs`，按锁表/comm/FD 目录/FD 链接、
   permission/other、非法 PID、非法模式共 10 类聚合，每类只保留计数和一个样本。
   十万条非法锁模式 + 失败 FD 的回归测试修复前返回 100001 条 issue，修复后
   issue 数低于严格上界且成功 FD 保留。
2. HIGH 锁路径错误：`fd_inode_map` 现在把 FD 目录和 readlink 失败写入同一有界
   诊断器；两条 `locks_of` 回归测试证明锁数据保留并返回 Partial。
3. MEDIUM PID 0：在构造 `FileLockEntry` 前执行 `Pid::new`；非法 PID 跳过并聚合
   为 `ParseFailed`，不再使用 `Pid::MIN` 回退。
4. MEDIUM 自证测试：删除 `core/tests/file_inventory_contract.rs`；保留并扩充真实
   Linux 合成 `/proc` 边界测试，core 真实消费者由既有 target/files 测试覆盖。

复验结果：`cargo test -p runquiry-core --locked` 107 passed；
`cargo test -p runquiry-platform --locked` 97 passed；严格 clippy、14 个自有 Rust
文件 rustfmt check、`git diff --check` 均 exit 0。真实受控 QA 再次观测到目标普通
FD 1、真实锁 1、`locks_of` 1，且进程/文件/临时目录清理完成。详细证据见
[P1 实现证据](batch4a-file-inventory-contract.md)。
