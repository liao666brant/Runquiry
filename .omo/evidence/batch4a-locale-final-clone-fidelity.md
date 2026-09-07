# Batch 4A locale-final：最终视觉复审

- 审查日期：2026-09-04（Asia/Shanghai）
- 审查性质：独立、只读；未修改产品、截图或运行进程。
- 任务：以当前源码和新鲜 `locale-final-*` 截图确认输入 placeholder 本地化修复，并确认工作区/选择/筛选/拖拽分栏及断点没有回退。

## 直接检查的当前工件

- 源码：`crates/runquiry-ui/src/shell/mod.rs:192-207`、`render.rs:42-115`、`render_evidence.rs:65-97`、`interactions.rs:205-216`。
- 新鲜 locale 截图（6/6）：`locale-final-en-initial.png`、`locale-final-zh-empty.png`、`locale-final-en-back.png`、`locale-final-en-pid-smoke.png`、`locale-final-1099.png`、`locale-final-960.png`。
- 运行报告：`.omo/evidence/batch4a-final-runtime-qa.md`；局部修复报告：`.omo/evidence/batch4a-input-placeholder-locale-fix.md`。
- 既有布局上下文仅作为待复验范围：`resizable-final-*.png`、`.omo/evidence/batch4a-resizable-evidence-fix.md`；不把它们作为本轮 locale-final 通过依据。

## 已确认

1. **placeholder 产品修复正确。** `AppShell::set_language` 在切换全局 locale 后更新两个既有输入实体：query 于 `shell/mod.rs:199-201`、filter 于 `:202-204`，并使用当前 `Window`。这避免重建实体，符合保留输入值和焦点的设计。
2. **本地化实景正确。** `locale-final-zh-empty.png` 中两个空 placeholder 分别是“输入名称、PID、端口、文件或容器”和“筛选当前表格”；`locale-final-en-initial.png` 与 `locale-final-en-back.png` 中两个 placeholder 均回到英文。新工件消除了上一轮 `Filter current table` 留在中文界面的实际缺陷。
3. **1099 紧凑布局的当前实景存在。** `locale-final-1099.png` 没有内联详情/分栏，主数据区占满；与 `render.rs:113-115` 一致。
4. **设计系统仍是活组件树。** 当前 `render.rs:96-109` 仍使用 `h_resizable`/`resizable_panel`，无图像或背景图替代界面；主题与文字仍从 token/翻译层取得。

## 发现

### HIGH

无已证实的当前产品缺陷。

### MEDIUM

无。

### [evidence] BLOCKER

1. **本轮没有完成请求所需的同一运行态完整复验，不能以 locale-final 截图宣称分栏、状态保留与 Sheet 全部通过。**
   - `locale-final-960.png` 显示的是无 Sheet 的 960×640 主表，故不证明“960 Sheet”；虽然 `locale-final-1099.png` 正确证明了 1099 compact，二者不能互相替代。
   - `locale-final-en-pid-smoke.png` 显示 Port 目标值 `240000` 的校验失败，而非可识别的行选择、非空 filter、保留的工作区或拖后 divider。`locale-final-zh-empty.png` 为正确的空 placeholder 状态，也没有保留这些状态的可观察证据。
   - `.omo/evidence/batch4a-final-runtime-qa.md` 将顶部旧二进制 SHA `22ab…` 与“最终构建 SHA `b085…`”并列，且表格仍保留语言 placeholder FAIL 的旧结论，随后文字又称 PASS。该报告不能消除上述截图缺口，也不构成可追溯的同构建 evidence ledger。
   - 必须补拍/重写证据（不需要改产品代码）：在 SHA `b085…` 当前二进制的一次连续真实窗口会话中，捕获并记录：
     1. 1280 宽窗先实际拖到非 65/35 位置，再切深色中文、回切英文，截图必须同时可观察 divider 位置、当前工作区、表格选中行和非空 filter/value；
     2. 清空 query/filter 后的中文 placeholder 与英文回切 placeholder；
     3. 960 下真实打开的 Sheet，以及 Escape 后的主表；
     4. 1099 无内联详情；
     5. SHA、窗口 ID、截图 SHA-256 和清理回执写入无矛盾的单份报告。

### LOW

无新增视觉 polish 项；既有 1100 初始表格末列需要横向滚动的观察仍属后续可用性证据，不是本次 locale 修复缺陷。

## 结论

- **recommendation：REQUEST_CHANGES（仅 evidence）。**
- **产品状态：**placeholder 国际化修复已由新鲜实景与源码确认；未发现其引入的设计系统或布局回归。
- **阻断原因：**当前 locale-final capture set 未包含请求的 960 Sheet、拖后状态保留和可观察 selection/filter/workspace；运行报告混有两个构建 SHA 和旧失败记录。
- **reportPath：**`.omo/evidence/batch4a-locale-final-clone-fidelity.md`

补齐上述同一构建会话的截图和一致报告后，应仅重新运行此独立视觉审查，不需要再修改已经确认正确的产品代码。
