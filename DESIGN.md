# Runquiry 设计规范（DESIGN.md）

Runquiry 唯一的设计规范。所有界面实现（工作区、调查面板、设置、gallery）都从这里取语义；
实现位于 `crates/runquiry-ui/src/`，其中 **原始色值只允许出现在
`crates/runquiry-ui/src/theme.rs` 的 `PALETTE` 表**。本文只写稳定语义；库版本、依赖
提交与运行状态由 Cargo.lock / Git 管理，不写在这里。

## 0. 设计读数（固定）

| 读数 | 取值 | 含义 |
|---|---|---|
| `DESIGN_VARIANCE` | **3** | 克制的产品个性：一个强调色 + 一套石墨表面，不做品牌化装饰 |
| `MOTION_INTENSITY` | **2** | 只允许短过渡（出现/消失/展开/关闭）与不定进度指示；禁止装饰性持续动画 |
| `VISUAL_DENSITY` | **9** | 高密度诊断控制台：紧凑行高、小字号、小间距，优先信息密度而非留白 |

## 1. 设计原则

1. **任务优先**：界面围绕「正在查看哪个对象、下一步能做什么」组织，不围绕组件清单。
2. **组合优先**：优先使用 gpui-component 既有组件（Sidebar、DataTable、Tree、Sheet、
   AlertDialog、Notification、StatusBar），不重复实现其行为；不使用自定义可点击 `div`
   替代语义组件。
3. **Token 优先**：调用点不出现裸色值、裸圆角、裸间距；一律 `cx.theme()` 语义 token
   或 rem 比例 helper（`p_2`/`gap_3`/`text_sm`/`h_8`）。
4. **桌面优先**：键盘可达、持久侧栏、密集数据表、独立滚动区域、明确的最小窗口尺寸。
5. **状态必须可见**：rest/hover/pressed/focus/selected/disabled/loading/error 各有独立、
   一致的呈现；状态不得只靠颜色表达。

## 2. 颜色

### 2.1 语义规则

- **中性石墨/灰色表面**承载全部信息：window、sidebar、popover、table、title bar、
  status bar。
- **钴蓝（cobalt）只作交互强调**：主按钮、选中行/列、焦点环、拖拽边界、链接强调、
  下拉目标。不得用于装饰、不得用于表达成功/警告/危险。
- **绿 / 黄 / 红 只表达语义状态**：success / warning / danger。沿用库内置默认色值，
  Runquiry 不重新定义；danger 只用于破坏性结果与真正的失败，warning 只用于警告与
  权限边界，success 只用于成功结果。
- 不以颜色作为唯一信息通道：每种状态同时有图标 + 文案（见 §6）。
- 每个自定义表面都要在浅色与深色两种模式下核验；不得假设 foreground 是黑、background
  是白。

### 2.2 主题（浅色 / 深色）

- 两套主题共用同一张 token 表（`theme::PALETTE`），同一 token 名在两种模式下取值不同
  （由单元测试 `light_and_dark_cover_the_same_tokens` 强制）。
- 默认跟随系统外观；gallery 与设置可显式切换，切换不得丢失当前工作区、筛选与选择。
- 主题名：`Runquiry Light` / `Runquiry Dark`。
- 切换主题必须同步 Base 层投影（滚动条、窗口边框跟随新主题）。

### 2.3 token 覆盖范围

| 组 | token |
|---|---|
| 表面 | `background`、`foreground`、`border`、`muted.background/foreground`、`secondary.*`、`popover.*`、`accent.*`、`input.border` |
| 交互强调 | `primary.background/hover/active/foreground`、`ring`、`selection.background`、`drag.border`、`drop_target.background`、`list.active.*`、`table.active.*` |
| 固定栏 | `sidebar.*`（background/border/foreground/accent/primary）、`title_bar.*`、`status_bar.*` |
| 数据区 | `table.background/even/head/head_foreground/hover/row_border`、`list.background/even/head/hover` |

未列出的角色（success/warning/danger/info、图表色、高亮主题）沿用 gpui-component 内置
浅/深主题。

## 3. 字体

- 界面文本：系统 UI 字体（`.SystemUIFont`），保证中英文覆盖，不打包字体文件。
- 代码、标识符、路径、快捷键、对齐数值：系统等宽字体（Linux `DejaVu Sans Mono`）。
- 中文字号偏小时可读性差，正文最小 `text_xs`；CJK 文本禁止全大写与字间距处理。
- 基准字号即窗口的 `rem`（由 `Root` 下发）：缩放时字号、间距、控件尺寸一起缩放。

## 4. 密度、间距、圆角

- 密度分层：默认 medium；工具栏、菜单、表格、重复数据用 compact（`small`）；
  空态与重大决策可用宽松留白。
- 间距阶梯：2 / 4 / 8 / 12 / 16 / 24 / 32 px（`xxs`→`xxl`），对应关系优先于绝对值：
  - 同一控件内部：`xxs`–`xs`（2–4）
  - 紧邻控件（按钮图标与文字、对话框动作）：`sm`（8）
  - 同一内容组（表单行、通知列）：`md`（12）
  - 同一节内的分组：`lg`（16）
  - 分节 / 主区块：`xl`–`xxl`（24–32）
- 布局代码禁止直接 `px(...)`；仅以下情况允许并注释原因：一设备像素的发丝线、平台窗口
  内衬、位图尺寸、命中容差、与外部表面对齐的几何。
- 圆角由主题给出（Runquiry：general 4px，Dialog/Notification 6px）；圆形与胶囊用
  `radius_full()`。
- 表格目标行高 34px（产品契约，模块 05）；行高与字号、内边距一起按密度分层调整。

## 5. 层级与表面

- 层级顺序（由低到高）：window 背景 → 标题栏/侧栏/状态栏 → 主数据区 → 详情区 →
  Popover/Menu → Sheet → Dialog/AlertDialog → Notification/Tooltip。
- 覆盖层（Sheet/Dialog/Notification）的绘制由内容视图负责，绘制顺序即层级
  （Notification 最后绘制，置于最上层）；根视图只协调覆盖层状态与焦点恢复，不代为绘制。
- 层内用背景对比与发丝线区分区域，不逐层加阴影；阴影只给真正的悬浮层。
- 同类表面共用同一处理（例如所有弹层共用 `popover` 表面）。
- 强调是有限预算：一个区域只有一个焦点；颜色、加粗、徽标、警示不同时叠加。
- 阅读顺序固定为：标题栏（窗口名 + 操作按钮）→ 侧栏 → 主数据 → 详情 → 状态栏；Tab 顺序与视觉顺序一致。

## 6. 状态语义与呈现规则（六种状态）

| 状态 | 语义 | 图标 | 颜色 | 交互规则 |
|---|---|---|---|---|
| `loading` | 采集进行中 | 不定进度指示（Spinner） | 中性 `muted_foreground` | 保留上下文，禁用重复提交；首样本未到不显示伪数值 |
| `empty` | 采集成功但结果为空 | `inbox` | 中性 | 说明下一步（调整筛选/重试）；不算错误 |
| `error` | 采集或分析失败 | `circle-x` | `danger` | 说明发生了什么 + 恢复动作（如「重试」） |
| `unsupported` | 平台无该能力 | `dash`（带圆形外框） | 中性 | 是能力边界，不是错误；不用错误 toast；保留导航入口并解释原因 |
| `unavailable` | 平台支持但当前环境不可用（采集器缺失、服务不可达） | `info` | 中性 | 是环境边界，不是平台缺陷也不是错误；解释原因，不误报为「可重试失败」 |
| `permission-denied` | 权限不足 | `eye-off` | `warning` | 是权限边界，不是故障；给出可行路径，不提示自动提权 |

- `Ready`（有数据）不使用状态组件，直接渲染数据。
- 实现入口：`runquiry_ui::StateView`（无状态 `RenderOnce`，标题/说明/动作由调用方传入）。
- 状态图标固定：loading=不定进度指示、empty=`inbox`、error=`circle-x`、
  unsupported=`dash`、unavailable=`info`、permission-denied=`eye-off`；图标色分别为
  中性 / 中性 / `danger` / 中性 / 中性 / `warning`。
- 部分成功不是 error：有数据的字段照常显示，失败项以 issue 呈现，不得丢弃其余数据。

## 7. 窗口与布局

- 默认窗口 **1280×800**，最小 **960×640**。
- 固定区域：标题栏、侧栏、状态栏；不随内容滚动。
- 标题栏与操作栏合并（gpui-component `TitleBar`）：原生标题栏透明，应用内绘制——
  窗口名与操作按钮同置左侧（组间距 16px），最小化/最大化/关闭由 `TitleBar` 自绘并经
  `WindowControlArea` 交给系统处理；按钮容器按下 `stop_propagation` 防误触拖拽。
- 侧栏固定宽 120px。
- 宽度 ≥1100px：65/35 可调整「列表-详情」布局。
- 宽度 960–1099px：详情改用 Sheet（右侧滑入），不压缩主数据区。
- 每个主要区域声明三个尺寸约束：仍可用的最小值、舒适默认值、剩余空间的去向。

### 7.1 滚动所有权

| 区域 | 滚动所有者 |
|---|---|
| 主数据表 | DataTable 自带的纵向/横向滚动（虚拟化，只渲染可见行） |
| 详情面板 | 详情区自己的独立纵向滚动，不与主表联动 |
| 树/列表 | 该组件自带的滚动区域 |
| 标题栏、侧栏、状态栏 | 不滚动 |
| 窗口本体 | 不滚动；禁止整窗滚动兜底 |

规则：一个滚动区域只有一个 owner；`flex_1()` 的子项必须配 `min_w_0()`/`min_h_0()`，
否则长内容会拒绝收缩；滚动条属于真正滚动的那个区域，贴其末缘。

## 8. 焦点与键盘路径

### 8.1 焦点规则

- 持有键盘交互的实体保留 `FocusHandle`；不得在 render 中无条件请求焦点。
- 焦点环必须可见：浅色、深色、窄窗口下都要可辨识；剪裁祖先不得吞掉外向焦点环。
- 覆盖层打开时转移焦点，关闭时恢复到触发控件；嵌套覆盖层从最上层开始关闭。
- `selected`（持久选中）与 `focused`（当前键盘目标）严格区分，不得混用。

### 8.2 键盘路径（产品契约）

| 键 | 行为 |
|---|---|
| `Tab` / `Shift+Tab` | 按 visual 顺序前进/后退；进入焦点陷阱（Sheet、Dialog）后循环不外溢 |
| `Ctrl/Cmd+K` | 聚焦调查栏 |
| `Ctrl/Cmd+R` | 刷新当前工作区 |
| `Ctrl/Cmd+1..4` | 切换 Processes / Ports / Containers / File Locks |
| 方向键 | 表格/树内移动选择 |
| `Enter` | 执行当前明确动作（打开详情、确认） |
| `Escape` | 关闭最上层 Sheet/AlertDialog，焦点返回触发控件 |

- `loading`/`disabled`/`unsupported` 状态不得产生误导性交互（按钮禁用、动作不可达、
  行为可预期）。
- 组件自带键盘能力接线后必须验证：DataTable（方向键/Tab/Home/End/PageUp/PageDown）、
  Tree（上/下/左/右）、Button（`Tab` 聚焦 + `Enter`/`Space` 触发）。
- 行为要求（与具体组件实现无关，必须成立）：
  - 侧栏、表格必须有完整键盘路径——侧栏导航与表格内的选择移动都可只靠键盘完成，
    并有可观察证据（焦点环、状态行读数或截图）。
  - 覆盖层（Sheet、AlertDialog）关闭时焦点必须恢复到触发控件；`Escape` 关闭最上层。
  - 窗口启动时必须把初始焦点放到视觉顺序的首个区域（标题栏操作按钮）。
- 随 gpui-component 版本变化的组件行为限制（如某组件不进入 Tab 序、不暴露焦点句柄）
  不在本文记录；当前锁定版本的清单见
  `docs/qa/a4-gallery/qa-report.md` 的「库版本相关已知限制」。

## 9. 可访问性

- 每个动作键盘可达、可操作；焦点顺序符合视觉与任务顺序。
- 图标按钮必须有 accessible name 与 tooltip；状态不得只用颜色表达。
- 文本与有意义的边界要保证对比度；disabled 与 read-only 可区分。
- 标签、错误、说明靠近其控件；长翻译与放大字号下仍可用（控件不得按单一语言定宽）。
- 动效尊重系统「减少动态效果」偏好；不要求看懂动画才能理解状态。

## 10. 文案（接口语言）

- 目的地与对象用名词（`Processes`、`Containers`），命令用动词（`Terminate`、`Refresh`）。
- 确认对话框：标题写决策本身（`Terminate "containerd-shim"?`），正文只写后果与恢复
  信息，按钮写结果动词（`Terminate`），破坏性结果用 danger 样式；不用 `Are you sure?`
  与 `OK`。
- 需要更多输入的命令以省略号 `…` 结尾（`Settings…`），立即执行的命令不加。
- 中文使用全角标点；短标签不加句末标点，完整说明句加。
- 状态是形容词/短语（`Loading`、`Permission denied`），不是句子。

## 11. 动效

- 允许：不定进度指示、出现/消失/展开的短过渡、滚动条淡入淡出。
- 禁止：装饰性持续动画、循环背景动效、为「显得活」而加的动画。
- 动效是解释变化的手段，不是状态本身；被打断的过渡应从当前值反向，而不是重播。

## 12. 组件实验台（gallery）边界

- gallery 只用于开发与视觉/键盘 QA：`cargo run --example gallery`，不进产品窗口，
  examples 不参与发布与安装包。
- 数据全部合成，绝不读取本机进程、端口、文件、容器或环境变量；必须包含长中文条目与
  无空格长路径条目。
- gallery 提供 `--size` / `--theme` / `--lang` / `--state` / `--open` 参数与窗口内
  切换控件，供 QA 脚本化驱动；产品设置持久化属于 B4，gallery 不做。
- 焦点指示：窗口底部状态行实时显示焦点位置，粒度为「区域级焦点名 + 可辨识的控件标识
  （`tab_index`）」，作为键盘 QA 的可观察证据。