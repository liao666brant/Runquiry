# Batch 4A 宽窗布局：独立视觉与设计系统审查

- 审查日期：2026-09-04（Asia/Shanghai）
- 审查性质：只读；未修改产品源码、截图或运行态。
- 目标：确认 Batch 4A 对 1280×800 宽窗主表被详情挤压的问题的修复，符合 `DESIGN.md` 与模块 05 的产品布局契约，且为真实、可扩展的 GPUI/gpui-component 界面。

## 已检查的工件

| 类别 | 工件 |
| --- | --- |
| 设计与验收契约 | `DESIGN.md` §1、§2、§3、§4、§7–§9；`.omo/plans/runquiry-gpui-desktop/05-desktop-ui.md` B5/B6 及“布局与交互验收” |
| 旧的失败实景 | `docs/qa/batch4a/screenshots/targets-final-name-pid.png`、`targets-final-port.png`、`targets-final-file.png`（均为 1280×800） |
| 修复后的实景 | `docs/qa/batch4a/screenshots/wide-layout-green-process-pid-1280.png`、`wide-layout-green-port-1280.png`、`wide-layout-green-file-1280.png`、`wide-layout-green-process-1099-compact.png`、`wide-layout-green-process-960-sheet.png` |
| 源码 | `crates/runquiry-ui/src/theme.rs`、`shell/render.rs`、`shell/render_workspace.rs`、`shell/render_detail.rs`、`shell/render_evidence.rs`、`shell/render_workspace_detail.rs`、`shell/render_issues.rs`、`shell/compact_detail.rs`、`processes/table.rs`、`workspaces/{ports_table,file_locks_table}.rs` |
| 既有 QA 说明 | `.omo/evidence/batch4a-wide-layout-fix.md`、`.omo/evidence/batch4a-final-targets-qa.md` |

## 视觉对比结论

旧的三张深色 1280×800 图确实显示了主区被压成近似单字符列、调查栏标签与标题互相覆盖的产品缺陷。三张新的宽窗图已消除该回归：侧栏右侧的主/详情分界约在 x=903，主表仍保留可扫描的列和行，右侧详情可读；新图与修复证据记录的 65/35 几何一致。

`wide-layout-green-process-1099-compact.png` 未渲染内联详情，主表占据可用工作区；`wide-layout-green-process-960-sheet.png` 展示真实右侧 Details Sheet，背景主表没有被压缩。英文主界面、中文系统诊断、长文件路径和详情中的中英混合文本均无 tofu、孤字或横向越界。浅色主题的石墨表面、钴蓝选中/焦点环、warning 面板语义也与 `theme.rs` 的集中 token 表一致。

源码不是截图或栅格背景的替代品：`render.rs` 组合 `Sidebar`、`Button`、`Input`、`StatusBar` 和 `Root` 的 Sheet 层；三张工作区表均为 `DataTable`；详情由真实 GPUI `div`/`v_flex` 树渲染。没有在 UI 源码中发现 raster/image/background-image 用于复刻界面。颜色调用点使用 `cx.theme()`，原始色值集中在 `theme.rs:23-73`。

## 发现

### HIGH

1. **[product] 宽窗分栏并不可调整，违反当前的明确布局契约。** `DESIGN.md:109` 和模块 05“布局与交互验收”明确规定“1100px 及以上为 65/35 **可调整**列表-详情布局”，而 `render.rs:74-81` 仅把两个面板并列；`render_workspace.rs:30-48` 与 `render_detail.rs:21-34` 只通过 `flex_1().flex_grow(65/35)` 分配固定比例。对 `crates/runquiry-ui/src` 和 `crates/runquiry-app/src` 的 `Resizable|resizable|resize` 搜索无命中，截图也没有拖拽分隔条或其他调整路径。当前修复正确地恢复了静态 65/35，不能据此宣称完成“可调整”布局验收。
   - 必须修复：使用 gpui-component 的真实 resizable pane 状态/分隔条完成分栏调整，并定义最小宽度、宽窗恢复规则和真实交互证据；或由产品方明确将契约收敛为固定 65/35 后同步 `DESIGN.md`、计划和测试。

2. **[product] 详情证据组仍可将无空格长路径/来源值裁切，而非按设计规则收缩或换行。** `render_detail.rs:31` 用 `overflow_x_hidden()` 为父面板兜底；但 `render_evidence.rs:65-89` 的根组和每个动态条目都没有 `min_w_0()` 或 `whitespace_normal()`。其中 `render_evidence.rs:52-63` 直接将 `lock.path.display()` 放入文本。这与 `DESIGN.md:119-133` 的“flex 子项必须配 min_w_0/min_h_0，长内容必须明确收缩、换行或滚动”相悖。当前 file 宽窗截图只证明调查 Input 中的路径和部分详情可见，未覆盖这个 `Analysis.file_locks` 证据路径；父级隐藏横向溢出时，该路径存在静默裁切风险。
   - 必须修复：为 evidence 根/组/动态条目建立与 `render_detail.rs:117-172` 和 `render_workspace_detail.rs:19-145` 一致的收缩与换行约束；以真实无空格长路径的 `Analysis.file_locks` 截图复验，确认内容不扩张主区且不被隐藏。

### MEDIUM

1. **[product] 进程表的列宽仍是散落的物理像素常量。** `crates/runquiry-ui/src/processes/table.rs:42-49` 使用 `px(280/160/88/160/120)`，没有与产品 token 层或缩放策略建立可见关联，也未记录 DataTable API 限制。`DESIGN.md:72-73` 要求普通布局避免 `px(...)`。这不会重现本轮主区塌缩，且 Ports/File 表没有同样写法，所以不把它视为本轮阻断；但需要在 DataTable 支持的前提下改用相对尺度或记录为 API 限制，并在字体缩放下复验列可读性。

2. **[evidence] 修复后的完整视觉集合没有覆盖深色 + 中文的宽窗详情。** 新的五张绿色截图覆盖浅色英文 1280、1099 和 960 Sheet；旧图虽是深色中文，但正是修复前的失败布局，不能为当前源码背书。主题 token 与本地化源码设计良好，故不是当前产品阻断；最终视觉门禁仍应补一张修复后深色中文宽窗详情实景，特别检查中文 warning/长路径的换行与焦点环。

### LOW

1. **[polish] 960 Sheet 截图中表格行上方保留了一个 `systemd` tooltip（`wide-layout-green-process-960-sheet.png`，约 x=365, y=390）。** 这不遮挡重要内容，且能证明截断命令存在 tooltip，但作为 Sheet 布局验收截图略分散注意力。下次可将鼠标移离数据行后再截取静态构图。

## 保持不回归的部分

- `flex_1()` 加 `flex_grow(MAIN_RATIO/DETAIL_RATIO)`（`render_workspace.rs:30-37`、`render_detail.rs:21-29`）将 basis 归零，修复了旧图的 intrinsic-size 挤压；三张新的 1280 实景均验证该结果。
- `min_w_0()`、`whitespace_normal()` 与详情独立垂直滚动（`render_detail.rs:28-33,117-172,246-258`；`render_workspace_detail.rs:19-145`）已经覆盖调查标题、普通字段、issue 和环境条目，英文/CJK 可读性明显恢复。
- 窄窗的 `open_sheet` 路径为真实 `window.open_sheet`（`shell/interactions.rs:205-216`），不是伪装成侧栏的静态图；1099 与 960 实景分别验证断点两侧。
- 主题为集中、可审计的语义 token 表（`theme.rs:23-73`），产品树复用 `gpui-component` 原语与 DataTable，而不是截图拼贴。

## 结论

- **recommendation：REQUEST_CHANGES**
- **blockers：**上述两项 HIGH。第一项是明确的“可调整 65/35”验收条件尚未实现；第二项是长无空格证据的收缩/换行契约未闭合，`overflow_x_hidden` 会把缺陷隐藏为不可见裁切。
- **reportPath：**`.omo/evidence/batch4a-wide-layout-clone-fidelity.md`

在解决两项 HIGH 后，应以同一构建重新捕获：可拖动后的宽窗 65/35（含极值最小宽度）、含 `Analysis.file_locks` 无空格长路径的详情、深色中文 1280，以及既有 1099/960 两个断点；然后由新的独立只读评审复核。
