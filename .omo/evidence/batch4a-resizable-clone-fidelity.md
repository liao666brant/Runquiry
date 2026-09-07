# Batch 4A 可调整分栏：复审视觉与设计系统报告

- 审查日期：2026-09-04（Asia/Shanghai）
- 审查性质：独立、只读；未修改 Rust 源码、运行进程或截图。
- 当前产物依据：`.omo/evidence/batch4a-resizable-evidence-fix.md` 所列二进制 SHA-256 `22ab2db801e9a62cf04947719478ff6f2e962337c6547312eee26aa679b4d2ca`。

## 已检查的工件

| 范围 | 工件 |
| --- | --- |
| 新鲜实景（完整集合，6/6） | `docs/qa/batch4a/screenshots/resizable-final-before-drag-1280.png`、`resizable-final-after-drag-1280.png`、`resizable-final-dark-zh-retained-drag-1280.png`、`resizable-final-1100-initial.png`、`resizable-final-1099-compact.png`、`resizable-final-960-sheet.png` |
| 修复证据 | `.omo/evidence/batch4a-resizable-evidence-fix.md` |
| 设计与任务契约 | `DESIGN.md` §1–§4、§7–§10；`.omo/plans/runquiry-gpui-desktop/05-desktop-ui.md` B5/B6 与布局验收 |
| 源码 | `crates/runquiry-ui/src/shell/render.rs`、`render_evidence.rs`、`render_detail.rs`、`render_workspace.rs`、`compact_detail.rs`、`interactions.rs`、`mod.rs`、`theme.rs` |

## 已确认通过的项目

- **真实可调整分栏：** `render.rs:96-109` 使用锁定 gpui-component 的 `h_resizable("workspace-detail-split")` 和两个 `resizable_panel()`，不是对截图或状态字段的伪造。`resizable-final-before-drag-1280.png` 的分隔约 x=910，`resizable-final-after-drag-1280.png` 为约 x=740；修复证据记录了同一次真实 X11 mouse down/move/up 拖拽。拖后主、详情均仍可读。
- **初始几何与断点：** `render.rs:42-49` 计算侧栏之外的 65/35 初值，且 `render.rs:287-295` 对 1100/1280 断言。`resizable-final-1100-initial.png` 与 1280 初始图均与该比例相符；1099 图没有 inline detail/divider，960 图为真实 Sheet。`interactions.rs:205-216` 证实窄窗是 `window.open_sheet` 的真实组件路径。
- **长证据文本：** `render_evidence.rs:65-97` 已在 evidence 根、分组和动态项上使用 `min_w_0().whitespace_normal()`。1280 初始、拖后与深色中文图都显示实际 `File locks` 无空格超长路径在详情宽度内换行，未再次挤压主区或横向裁切。
- **真实设计系统：** 树由 `Sidebar`、`Button`、`Input`、`DataTable`、`StatusBar`、`Root` Sheet 层和 `Resizable` 组成；未发现 raster/screenshot/background-image 替代活组件。调用点读取 `cx.theme()`，原始色值集中在 `theme.rs:23-73`，浅深主题和 warning/selection 语义与 `DESIGN.md` 一致。
- **视觉质量：** 修复后图中英文与中文诊断、长路径、钴蓝焦点/选中、深色表面均清晰，无 tofu、单字符列或详情溢出。1100 图最右 Health 列在初始视图只露出窄片，但该表使用 `DataTable::scrollbar_visible(true, true)`；这符合 `DESIGN.md` 对主表拥有横向滚动的约定，作为紧凑宽度下的密度观察，不单列阻断。

## 发现

### HIGH

无。

### MEDIUM（阻断当前“中英文重渲染”验收）

1. **[product] 切换为中文后，两个持久输入框的 placeholder 没有重新本地化。** `resizable-final-dark-zh-retained-drag-1280.png` 的空筛选输入仍显示英文 **“Filter current table”**，同时侧栏、表头、状态栏和调查按钮均已是中文；这说明不是刻意双语文案。词条本身存在于 `crates/runquiry-ui/locales/app.yml:141-143`（`zh-CN: 筛选当前表格`）。根因也可由源码直接确认：`AppShell::set_language` 只调用 `self.data.relocalize(cx)`（`shell/mod.rs:192-201`），而 `query_input`/`filter_input` 只在创建时取 placeholder（`shell/mod.rs:135-139`），没有更新路径。
   - 必须修复：语言切换时更新两个现存 `InputState` 的 placeholder，同时保留用户已输入的 value、筛选、选择和可调整分栏状态；补一张深色中文、空 query 与空 filter 的新鲜实景及相应回归测试。

### LOW / 后续观察

1. `resizable-final-1100-initial.png` 的左侧表格最后一列在初始视区只显示部分字符，横向滚动是设计允许的所有权路径；后续键盘/交互 QA 可明确留一张 1100 横向滚动到末列的证据，以免将“可横滚”误读成固定列完整可见。

## 结论

- **recommendation：REQUEST_CHANGES**
- **blockers：**没有剩余分栏或长路径布局阻断；唯一阻断是上述 MEDIUM 的语言切换后输入 placeholder 未刷新。它直接违背 UI 层对 rust-i18n 运行时切换的契约，也使本任务要求的“dark + 中文 rerender”不完整。
- **reportPath：**`.omo/evidence/batch4a-resizable-clone-fidelity.md`

修复该本地化缺口并以同一构建补拍后，重新独立审查即可关闭本轮视觉门禁；已经通过的可拖拽 65/35、长路径换行、1099 compact 和 960 Sheet 不应回退。
