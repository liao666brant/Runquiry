# Batch 4A UI 模块边界脚手架证据（重新核验）

- 场景：`runquiry-ui` 导出 `processes` 与 `workspaces`，且两个模块文件只包含模块级文档注释。
- 结构调用：`rg -n '^pub mod (processes|workspaces);$' crates/runquiry-ui/src/lib.rs`，并断言两个 `mod.rs` 各为 1 行且各含 1 个 `//!`。
- 结构可观察结果：匹配 `12:pub mod processes;`、`18:pub mod workspaces;`；`module-only assertions: PASS`；退出码为 `0`。
- 构建调用：`cargo check -p runquiry-ui --locked`
- 构建可观察结果：退出码为 `0`；输出为 `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 0.36s`。
- 格式调用：`git diff --check -- crates/runquiry-ui/src/lib.rs crates/runquiry-ui/src/processes/mod.rs crates/runquiry-ui/src/workspaces/mod.rs`
- 格式可观察结果：退出码为 `0`；输出为 `diff-check: PASS`。
- 范围调用：`git status --short -- crates/runquiry-ui/src/lib.rs crates/runquiry-ui/src/processes/mod.rs crates/runquiry-ui/src/workspaces/mod.rs .omo/evidence/batch4a-ui-module-boundary-scaffold.md`
- 范围可观察结果：三个源码目标分别显示 `M`、`??`、`??`，证据文件显示 `??`；未检查或修改其他源码路径。
- 手工 QA：N/A；本次仅增加模块声明与模块级文档，无运行表面。
- 对抗项：dirty worktree 适用，已仅检查分配路径；malformed input、prompt injection、cancel/resume、stale state、hung commands、flaky tests、misleading output、repeated interruption 均 N/A。
- 备注：`omo ulw-loop status --json` 返回 `EACCES`（`/home/syspetro/.local/bin/omo-ulw-loop`），因此无 active attempt 目录可用，证据保存在本目录。
