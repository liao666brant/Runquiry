# Batch 4A 最终代码审查

- 日期：2026-09-04（Asia/Shanghai）
- 审查性质：独立、只读代码审查；本文件是唯一新增审查工件，未修改产品源码。
- 结论：`codeQualityStatus: WATCH`；`recommendation: APPROVE`；`blockers: 无`。

## 审查指纹与范围

- 当前 `HEAD`：`e0bf6617d0541e8aae0df644699982c0f073bb02`。
- 当前工作区是未提交 Batch 4A。`git diff --stat HEAD`：23 个已跟踪文件、907 additions、416 deletions；另有本批次模块、测试、QA 和 evidence 的未跟踪文件。此次审查以完整工作区而非仅 Git diff 为准。
- 本次构建产物：`target/debug/runquiry` SHA-256 为
  `b085bfa101fc617ace1f1126487af9458b6d2268efb636320a22e400fbdaedc3`。
- 核心文件 blob SHA：`backend.rs` `b01731faffbb46fe5eb8eb9c409af2c0447f4eb4`；
  `backend/container.rs` `2a42da2c29a7ef661d32f1a348f5f7f058717a06`；
  `shell/mod.rs` `ac21111db875b7cb54cd72fc37d38452adb4c2ba`；
  `shell/render.rs` `41c1f93d9fb51ae2643dd5aa9a2fe7c523d81bae`；
  `shell/render_evidence.rs` `30f27e53df16b7449346e3df700a36520328156d`。
- 未发现根 `Cargo.toml`、`Cargo.lock`、`deny.toml`、`rust-toolchain.toml` 或 `witr/` 的跟踪/未跟踪越界变更。

## 本轮关闭的阻断项

1. **端口到容器 fallback**：`crates/runquiry-app/src/backend.rs:172-176` 在
   `SocketOwnerUnknown` 时进入 `resolve_published_port_container`；
   `backend/container.rs:19-27,84-108` 保留无已验证 host PID 的容器为
   `InvestigationTarget::Container`，并传递 `published_on` 诊断。回归测试
   `backend/container.rs:143-189` 验证唯一 published 容器、无 PID 和
   `ExternalToolFailed` diagnostic 均保留。
2. **宽窗可调整分栏与长证据**：`crates/runquiry-ui/src/shell/render.rs:96-109`
   使用 keyed `h_resizable("workspace-detail-split")` 和两个 `resizable_panel`。
   组件实现的运行时尺寸/basis 由 `ResizableState` 负责，调用方没有覆写保留样式；
   初始 65/35、360px/280px 最小值在 `render.rs:42-49,287-295` 受约束。
   `shell/render_evidence.rs:65-97` 对根、分组与动态值施加
   `min_w_0().whitespace_normal()`，不会以父级横向裁切隐藏长路径。
3. **语言切换 placeholder**：`crates/runquiry-ui/src/shell/mod.rs:193-207` 现在接收
   当前 `Window`，对既有 query/filter `InputState` 调用 `set_placeholder`；
   `shell/render.rs:186-197` 两个语言按钮传入该窗口。组件 API 仅替换 placeholder 并
   `notify`，没有重建实体或修改 value/focus，因此不会重置输入值、筛选、选择、会话或 keyed
   `ResizableState`。
4. **fresh 平台与分析单飞**：app 后端按 load/resolve/analyze 操作新建 LinuxPlatform，避免
   构造时 PID 基线遗漏新目标；`AnalysisGate` 仅串行完整 analyze，从而保留跨 fresh 实例的
   systemd 单飞界限。该后端仍只在 app 装配层依赖 platform；UI 仅依赖 core/WorkspaceBackend。

## 自动门禁（本审查实际复验）

```text
cargo test -p runquiry-core --locked       # 107 passed
cargo test -p runquiry-platform --locked   # 97 passed
cargo test -p runquiry-ui --locked         # 48 passed
cargo test -p runquiry-app --locked -- --test-threads=1  # 21 passed
cargo check -p runquiry-app --locked       # pass
cargo fmt --check                          # pass
git diff --check HEAD                      # pass
cargo clippy -p runquiry-core -p runquiry-platform -p runquiry-ui -p runquiry-app \\
  --all-targets --locked -- -D warnings -A clippy::multiple_crate_versions \\
  -A clippy::print_stderr                   # pass
```

最后两项 Clippy 放行是仓库既有锁定依赖多版本与 A1 `eprintln!` 基线例外，不是 Batch 4A
静默新增的 warning。UI 没有 `/proc`、外部命令、OS API 或未类型化逃逸；未发现生产代码中的
`unwrap`、`expect`、`panic!`、`todo!` 或 `unimplemented!`。

生产纯代码行计数：`backend.rs` 202、`backend/container.rs` 175、`shell/mod.rs` 192、
`shell/render.rs` 225（不含测试模块）、`shell/render_evidence.rs` 92；均不超过 250。

## remove-ai-slops / programming 视角

两种视角均已执行。未见删除式/同义反射测试、无用解析或归一化、未类型化 escape hatch、
平台逆向依赖、死依赖或超大生产模块。`ProcessCommand::bindings()` 的常量自证测试仍是低价值
implementation-mirroring 测试，但真实 app 键位已消费对应绑定，故列为非阻断维护风险。

## 残余风险与引用证据

- 100k 行只有索引/Arc 逻辑测试，未声称完成真实 100k 视觉性能实测。
- 本代码审查没有亲自执行运行时或视觉 QA。真实 Linux/WSLg 产品路径见
  `.omo/evidence/batch4a-final-runtime-qa.md`；可调整分栏、深色中文和长路径视觉复审见
  `.omo/evidence/batch4a-final-visual-review.md`；placeholder 最终修复的代码/门禁记录见
  `.omo/evidence/batch4a-input-placeholder-locale-fix.md`。这些报告是单独 QA 证据，不应被解释为
  本审查者亲自观察的结论。
