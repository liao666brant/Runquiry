# Batch 2 修复后视觉与设计系统验收

## 结论

- recommendation: **APPROVE**
- 覆盖：`docs/qa/a4-gallery/screenshots/batch2-fix/` 的 9/9 张 PNG；全部为当前渲染
  源码之后生成的完整 RGB PNG，尺寸与命名的 viewport 一致。

## 检查结果

| 范围 | 判定 | 证据 |
| --- | --- | --- |
| 1280×800、Light/English（Processes、Ports、Containers、File Locks） | PASS | 四张截图均保留侧栏、主区和详情区；主/详情分隔线约在内容区的 65% 处，详情占余下约 35%。四个工作区只有选中导航及状态栏对象名变化，层级稳定。 |
| 960×640、Dark/中文（四个工作区） | PASS | 四张截图仅有侧栏与完整主区，未保留压缩的内联详情栏或分隔线；状态视图位于完整主区中央，符合窄窗口优先保留主任务的规则。 |
| Unsupported 呈现 | PASS | 960 截图均显示中性圆框短横、`采集器不可用` 和能力边界说明；不使用危险色，也没有伪造 loading 数值。源码由 `DataState::Unsupported` 和 `StateView` 的中性色/圆框分支共同保证。 |
| 主题、密度、表面层级与 CJK | PASS | 浅色采用灰白表面、深色采用石墨表面；钴蓝仅用于选中态。中文标题、状态说明和状态栏均未裁切、未出现异常断行、缺字或字体度量漂移。 |
| gallery 第二行 Sheet | PASS | `gallery-row2-sheet.png` 中第二行 `containerd-shim-runc-v2` 被选中；右侧 Sheet 显示同一名称、完整换行路径、`PID: 2210`、`Port: 8080`。源码以当前 `TableState` 的 selected row 映射到同一合成行，未回退首行。 |
| 真实组件树与 token 驱动 | PASS | 检查了 `render.rs`、`state_view.rs`、`theme.rs` 和 gallery 的 table/overlay 实现：UI 由 GPUI/gpui-component 的 Sidebar、Button、StatusBar、StateView、DataTable、Sheet 和 Root overlay layer 实时组合；没有图片/背景图替代 UI。颜色集中在 `theme.rs::PALETTE`，调用处使用 `cx.theme()`；唯一 `px` 均注明为状态图标、表格列等几何边界。 |

## Findings

### CRITICAL

无。

### HIGH

无。

### MEDIUM

无。

### LOW

无。

## 残余风险

产品壳层尚未接入可选中的真实采集结果，因此 960–1099px 下“选中一行后从 Sheet 查看详情”的交互须在 B5/B6 接入数据表与选择后补做真实窗口验收；当前 9 张证据已经证明此阶段的窄宽度不压缩主区，且 gallery 已证明 Sheet 与选中第二行绑定正确。

## 已检查的工件

- 9 张截图：`docs/qa/a4-gallery/screenshots/batch2-fix/*.png`
- 设计契约：`DESIGN.md` §2、§5、§6、§7
- 活组件/布局：`crates/runquiry-ui/src/shell/render.rs`、`crates/runquiry-ui/src/state_view.rs`
- 主题 token：`crates/runquiry-ui/src/theme.rs`
- gallery 选择与 Sheet：`crates/runquiry-app/examples/gallery/{data,table,overlays,main}.rs`
