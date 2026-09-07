# Batch 4A AI 索引增量核验

- 核验时间（UTC）：2026-09-04T12:22:04Z
- 核验基线：`e0bf6617d0541e8aae0df644699982c0f073bb02`
- 变更范围：`AGENTS.md`、`crates/runquiry-{core,platform,ui,app}/AGENTS.md`。
- 未修改范围：五个 `CLAUDE.md`、Rust 源码、计划、状态、根配置、`Cargo.lock` 与 `witr/`。

## 场景与结果

1. 场景：Claude Code 指针与 Batch 4A 事实可被 Agent 确定性读取。
   - 调用：对五个 `CLAUDE.md` 逐一断言内容为 `@AGENTS.md`；以 `rg -F` 检索 `Batch 4A`、`FileInventory`、`AnalysisGate`、`h_resizable`、`InputState` 及最终测试基线。
   - 可观察结果：`claude_pointers=pass`、`fact_lines=19`；事实涵盖 P1/B5/B6/I1、core 107/platform 97/ui 48/app 21、fresh `LinuxPlatform` 与共享 `AnalysisGate`。
2. 场景：过期的 B5/B6 未实施表述不再误导后续实现。
   - 调用：`rg -F` 检索 `真实数据工作区属 B5/B6`、`Sheet 交由 B5`、`未做：B5/B6`。
   - 可观察结果：`stale_b5_b6_text=absent`。
3. 场景：索引文档没有破坏工作树，也未越过受限范围。
   - 调用：`git diff --check`；断言完整 HEAD 为上述基线；检查 `Cargo.toml`、`Cargo.lock`、`deny.toml`、`rust-toolchain.toml` 与 `witr/` 的 diff 名单为空；断言根 `AGENTS.md` 最后一行是 `- 扫描进度：已完成`。
   - 可观察结果：`diff_check=pass`、`head=pass`、`protected_scope=pass`、`index_status_last=pass`。

未运行构建或测试：本次仅更新 Agent 文档，且实现验证由 Batch 4A 的独立证据负责。
