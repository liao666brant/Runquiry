# Batch 4A 最终视觉 / Clone-Fidelity 门禁

- 审查日期：2026-09-04（Asia/Shanghai）
- 审查性质：独立、只读；未修改产品源码、截图或运行态。
- 审查构建：`target/debug/runquiry` SHA-256 `b085bfa101fc617ace1f1126487af9458b6d2268efb636320a22e400fbdaedc3`。本轮在工作区再次计算 SHA-256，结果完全匹配。

## 检查范围

| 维度 | 已检查工件 |
| --- | --- |
| 当前连续 WSLg 运行证据 | `.omo/evidence/batch4a-final-runtime-qa.md` 的 proof3/proof4 会话、SHA 与清理回执 |
| 1280 状态保留与拖拽 | `final-qa-proof3-selected-filter.png`、`final-qa-proof3-dragged-filter.png`、`final-qa-proof3-dark-zh-state.png`、`final-qa-proof3-light-en-state.png` |
| placeholder 双语 | `final-qa-proof3-zh-empty.png`、`final-qa-proof3-en-empty.png` |
| 响应式 | `final-qa-proof3-1099-compact.png`、`final-qa-proof4-960-sheet-open.png`、`final-qa-proof4-960-sheet-after-escape.png` |
| 当前 build 调查 smoke | `final-qa-proof-pid.png`（SHA `4b9e514bd4f7502e2784a24274d9a4fecb63756886855472ad89db1344e0e40a`）、`final-qa-proof-port.png`（SHA `26b220ac024ca556807ed5113c121f4441664506176b999f2f9dee2617c814b0`）、`final-qa-proof5-file-complete.png`（SHA `06241ac9e5915bd01122afe2b3559cc16f34182411c2c1fd279badf6fc379a2f`） |
| 源码 | `crates/runquiry-ui/src/theme.rs`、`shell/render.rs`、`shell/mod.rs`、`shell/render_detail.rs`、`shell/render_evidence.rs`、`shell/compact_detail.rs`、`shell/interactions.rs` |

## 核验结果

| 项目 | 结论 | 精确证据 |
| --- | --- | --- |
| 真实组件树与设计系统 | PASS | `render.rs:96-109` 组合 `h_resizable("workspace-detail-split")` 与两个 `resizable_panel()`；侧栏、表格、输入、StatusBar 与 Sheet 均为 GPUI/gpui-component 活组件。未发现 raster/image/background-image 用来替代 UI。`theme.rs:23-73` 集中原始色值，调用点以 `cx.theme()` 语义 token 呈现。 |
| 初始与拖后分栏 | PASS | `selected-filter` 为约 x=909 初始 65/35；真实 X11 drag 后 `dragged-filter` 为约 x=584。`dark-zh-state` 和 `light-en-state` 中分界仍在拖后位置，主区、详情、选中行及非空 filter=`systemd` 均仍可观察。 |
| 中英文 placeholder | PASS | `zh-empty` 的 query/filter 分别为“输入名称、PID、端口、文件或容器”/“筛选当前表格”；`en-empty` 均回到英文。源码 `shell/mod.rs:193-205` 就地更新两个既有 `InputState` placeholder，未重建实体。 |
| CJK、主题、状态可读性 | PASS | `dark-zh-state` 展示深色中文的选中、filter、loading 分析和详情分栏，未出现 tofu、孤字、单字符主列或横向溢出。错误/partial 警示保留文本与颜色以外的数量/说明。 |
| 长路径详情 | PASS | 当前 SHA 的 `final-qa-proof5-file-complete.png`（SHA `06241ac9e5915bd01122afe2b3559cc16f34182411c2c1fd279badf6fc379a2f`）显示 `rqFILE5 · PID 258791`、实际绝对路径及 `Flock/Write`，在详情宽度内完整可读。`render_evidence.rs:65-97` 的根、组和动态条目均明确 `min_w_0().whitespace_normal()`，不是依赖父级裁切。 |
| 960 Sheet 与 Escape | PASS | `proof4-960-sheet-open` 有真实 Details Sheet、关闭控件与 `systemd · PID 1`；`proof4-960-sheet-after-escape` 回到无 Sheet 的完整主表，选中行仍可见。源码路径是 `window.open_sheet`（`interactions.rs:205-216`）。 |
| 1099 紧凑断点 | PASS | `proof3-1099-compact` 没有 inline detail/divider，主表占满工作区；与 `render.rs:113-115` 一致。 |
| PID/Port/File 调查 | PASS | 当前 SHA proof `final-qa-proof-pid.png` 显示 `rqPROOFu · PID 244990`，`final-qa-proof-port.png` 显示 `nc · PID 244994` 与 `Tcp 127.0.0.1:37627 · LISTEN`，`final-qa-proof5-file-complete.png` 显示 `rqFILE5 · PID 258791`、`/tmp/runquiry-file-final-target.lock` 与 `Flock/Write`。三图哈希已在“检查范围”逐项复核。 |

## Findings

### CRITICAL

无。

### HIGH

无。

### MEDIUM

无。

### LOW

1. `final-qa-proof4-960-sheet-open.png` 仍保留位于表格行上的 `systemd` tooltip。它没有遮挡 Sheet 的标题、关闭控件或详情信息，属于截图构图上的轻微干扰，不是产品阻断。
2. 1100px 初始宽窗下，进程表最后列需要依赖 DataTable 的横向滚动才能完整检查；这是 `DESIGN.md` 明确允许的主表滚动所有权，建议后续补一张横向滚到末列的键盘 QA 图，但不阻断本次门禁。

## 判定

- **recommendation：APPROVE**
- **reportPath：**`.omo/evidence/batch4a-final-visual-review.md`
- **blockers：**无。

此前 `REQUEST_CHANGES` 报告仅记录其时的缺口；本报告仅针对当前 SHA `b085bfa...` 的 proof3/proof4 证据作最终判定。
