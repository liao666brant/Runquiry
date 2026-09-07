# Runquiry Batch 4A 最终 Gate Review

- recommendation: **APPROVE**
- goalId: `runquiry-batch4a-20260904`
- reviewedHead: `e0bf6617d0541e8aae0df644699982c0f073bb02` + 当前未提交工作区
- reviewedBinarySha256: `b085bfa101fc617ace1f1126487af9458b6d2268efb636320a22e400fbdaedc3`
- blockers: `[]`

## originalIntent

完成 Batch 4A 的 B5 Processes 与调查工作区、B6 Ports/Containers/File Locks 工作区，
并通过 app 装配层接入 Linux 真实平台能力；交付可由用户实际操作的 GPUI 桌面闭环，保留
稳定身份、generation、partial/unsupported、脱敏和响应式交互契约，不进入 B7/C3 或提交。

## desiredOutcome

四工作区和五种调查目标在真实 Linux GPUI 窗口可达；宽窗详情可调整，窄窗使用 Sheet；
异步结果不能污染新状态；平台采集不阻塞 UI；所有自动门禁、独立代码审查、运行 QA 与
视觉 QA 可由同一最终构建和持久工件交叉核验。

## userOutcomeReview

已满足。当前源码实现并由真实窗口证明 Processes、Ports、Containers 能力态、File Locks、
PID/Port/File 调查、双语 placeholder、主题/语言状态保留、可拖拽分栏、1099 紧凑布局及
960 Sheet/Escape。File 完成态补证 `final-qa-proof5-file-complete.png` 明确显示目标 PID、
绝对路径和 `Flock/Write`，其 SHA-256 为
`06241ac9e5915bd01122afe2b3559cc16f34182411c2c1fd279badf6fc379a2f`。

## Criteria audit

| Criterion | Verdict | Evidence |
| --- | --- | --- |
| C1 P1/B5/B6/I1 用户可达与领域边界 | PASS | `batch4a-file-inventory-contract.md`、`batch4a-b5-processes.md`、`batch4a-b6-workspaces.md`、`batch4a-shell-integration.md` |
| C2 稳定 identity/generation、脱敏、partial/unsupported | PASS | `runquiry-batch4a-final-code-review.md`；`crates/runquiry-ui/src/backend.rs`、`processes/redaction.rs`、`shell/refresh.rs` |
| C3 Port 到 published container fallback | PASS | `batch4a-port-container-fallback-fix.md`；`crates/runquiry-app/src/backend.rs`、`backend/container.rs` |
| C4 fresh LinuxPlatform + shared AnalysisGate | PASS | `runquiry-batch4a-target-backend-code-review.md`；`crates/runquiry-app/src/backend.rs`、`backend/analysis_gate.rs` |
| C5 GPUI 响应式、Resizable、Sheet、双语 InputState | PASS | `batch4a-final-runtime-qa.md`、`batch4a-final-visual-review.md`、当前源码 |
| C6 自动门 | PASS | 本 gate 实跑 core 107、platform 97、ui 48；app 沙箱外 21；check、严格 clippy、fmt、diff-check 均通过 |
| C7 依赖与参考源码边界 | PASS | `git status/diff`：根 Cargo、Cargo.lock、deny、toolchain、`witr/` 无变更；UI 无平台依赖/调用 |
| C8 生产模块规模 | PASS | 本轮新增/重写生产模块均不超过 250 pure LOC；测试模块不计生产上限 |
| C9 证据一致性与资源清理 | PASS | 最终 binary/report hash 一致；proof3/proof4/proof5 专属目录、PID、目标文件现场均 absent |

## Direct remove-ai-slops / programming pass

独立复查了完整工作区而非只看已跟踪 diff。未发现删除式测试、为请求删除而写的测试、
自然语言 pin、无用解析/归一化、未类型化 escape hatch、平台逆向依赖、生产死代码或超大生产
模块。高基数 FileInventory 诊断使用固定类别聚合，真实 Linux adapter 测试覆盖 PID 0、受限
FD、partial 与 100k 错误输入。`ProcessCommand::bindings()` 常量测试属于低价值
implementation-mirroring，但生产 app 真实消费该映射；它不违反本批次用户行为标准，列为 NOTE。

最终代码审查 `batch4a-final-code-review.md` 明确包含相同的 remove-ai-slops/programming
覆盖，并记录该弱测试与 100k 视觉边界；报告覆盖与本 gate 独立检查一致。

## Checked artifact paths

- `.omo/plans/runquiry-gpui-desktop.md`
- `.omo/plans/runquiry-gpui-desktop/05-desktop-ui.md`
- `.omo/start-work/todos.json`
- `.omo/start-work/ledger.jsonl`
- `.omo/evidence/batch4a-final-code-review.md`
- `.omo/evidence/batch4a-final-runtime-qa.md`
- `.omo/evidence/batch4a-final-visual-review.md`
- `.omo/evidence/runquiry-batch4a-20260904-code-review.md`
- `.omo/evidence/runquiry-batch4a-b5-processes-code-review.md`
- `.omo/evidence/runquiry-batch4a-b6-workspaces-code-review.md`
- `.omo/evidence/runquiry-batch4a-i1-rereview.md`
- `.omo/evidence/runquiry-batch4a-target-backend-code-review.md`
- `docs/qa/batch4a/screenshots/final-qa-proof5-file-complete.png`
- 当前 `git status`、完整已跟踪/未跟踪 Rust 源码与测试

## Exact evidence gaps / non-blocking residuals

1. 100k 行仅有 Arc/索引/可见范围逻辑测试，没有真实 100k 行 GPUI 视觉性能实测；现有报告已
   明确标为 `NOT RUN / residual`，本 gate 不把它表述为视觉通过。
2. `ProcessCommand::bindings()` 单测只验证常量映射，真实生产消费由源码与手工快捷键 QA 证明；
   后续可用 app action/键盘端到端测试替换。
3. 无容器运行时环境下仅验证诚实的 unavailable 能力态；真实容器引擎的视觉矩阵不属于本次
   Linux 数据闭环的阻断标准。

结论：可以将 B5、B6、I1、V1、V2、V3 标记为完成；C3/B7 仍保持原计划状态。
