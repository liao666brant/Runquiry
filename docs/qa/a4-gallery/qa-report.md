# A4 Gallery 视觉与键盘 QA 报告（独立复验）

复验人：A4 视觉与键盘 QA。日期：2026-09-03。
范围：只做真实视觉与键盘验收，不改代码；证据全部来自 `xdotool` 真实按键注入 + `import -window <窗口>` 截图。

- 被测对象：`crates/runquiry-app/examples/gallery`（`cargo build --example gallery --locked` 后的 `target/debug/examples/gallery`）
- 与实现者自证的差别：本轮是独立复验，截图与按键序列全部重新执行，未复用实现者首轮记录的旧图（该批旧图已被本复验集覆盖并移除）
- 截图目录：`docs/qa/a4-gallery/screenshots/{combo,states,overlays,keyboard}/`
- 环境：WSLg（X11 后端，`env -u WAYLAND_DISPLAY` 启动），屏幕 2575x3040

## 0. 环境备注（工具链差异）

- 本机的 ImageMagick 7 `import` 抓 root 窗口会报
  `missing an image filename`（XWayland 的 root 不参与合成，`import -window root` 无内容）。
  改用 **`import -window 0x<窗口ID>`** 按窗口 ID 抓取（xdotool 定位 `Runquiry Gallery` 后取
  `printf %x` 的 ID），每次都能拿到窗口位图。
- 每个用例前后都 `pkill -x gallery`，结束时确认无残留进程。
- 启动日志只有 libEGL/DRI3 软件渲染警告（AGENTS.md 已注明属正常），无 panic、无错误输出。

## 1. 基础组合（8 张，2 尺寸 x 2 主题 x 2 语言）

`combo/combo-<尺寸>-<主题>-<语言>.png`，全部由真实启动 + 激活 + 窗口截图产出：

| 截图 | 检查点 | 结论 |
|---|---|---|
| combo-960x640-light-en / -dark-en | 最小宽度下英文工具栏 | 换行成两行（数据状态一行、覆盖层一行），无裁切 |
| combo-960x640-light-zh-CN / -dark-zh-CN | 最小宽度下中文工具栏 | 同上，中文更长但仍只换行不裁切 |
| combo-1280x800-light-en / -dark-en | 默认尺寸 | 工具栏一行放下，覆盖层按钮完整 |
| combo-1280x800-light-zh-CN / -dark-zh-CN | 默认尺寸中文 | 一行放下，无裁切 |

深浅两套主题的石墨表面 / 钴蓝选中态在四个组合里都成立；状态行右下角主题-语言读数与启动参数一致。

## 2. 长中文与无空格长路径

- 长中文条目（组合截图即可见，8 张都有）：`数据库连接池维护守护进程（长名称示例）`、
  `容器运行时快照同步与清理调度器`、`本地缓存压实与索引重建服务（夜间批处理）`——
  名称列在列宽内省略号截断，中文不与相邻列重叠。
- 无空格长路径：`combo-1280x800-*` 中
  `/var/lib/containerd/io.containerd.runtime...`（省略截断）、
  `/run/user/1000/app/org.example.LongRunnin...`（省略截断）；
  **完整不截断的证据**在 Sheet：`overlays/open-sheet.png` 与 `open-sheet-dark-zh.png`
  中 `/usr/lib/postgresql/16/bin/postgres-checkpoint-worker` 换行完整展示。
- 树区：`org.example.LongRunningServiceName@instance.service`（长无空格 token）完整可见。

## 3. 五状态

`states/state-<state>.png`（1280x800 light en），另加 `state-loading-dark-zh.png`（dark zh-CN）：

| 状态 | 截图 | 结论 |
|---|---|---|
| loading | state-loading.png | 表格内建骨架（列结构保留），树区显示转圈 +「Collecting process data」，note 文案明示操作不可用 |
| empty | state-empty.png | inbox 图标（中性色），0 行 |
| error | state-error.png | 红色 circle-x + danger 文案，0 行 |
| unsupported | state-unsupported.png | 中性短横 + 圆形外框（复验后修正；本报告拍摄时为裸短横，见 §7 问题 #4） |
| permission-denied | state-permission-denied.png | 琥珀色 eye-off + warning 语义 |

四态（empty/error/unsupported/permission-denied）表格行数为 0，树区与表格区显示同一状态组件（替代而非叠加），
状态行同步显示 `Data state: <状态>`。

## 4. 覆盖层

`overlays/open-<overlay>.png`（light en），另加 `open-sheet-dark-zh.png`：

- **Sheet**：右侧滑入，标题 Process detail，正常态显示详情摘要（名称 / 完整路径 / PID / Port），
  右下 Close 按钮；不影响表格与侧栏。
- **AlertDialog**：居中模态，命名对象（"containerd-shim"）与后果，Cancel + danger `Terminate`。
- **Notification**：右上角，success 图标 + 标题（正常态回退为 gallery 标题），`autohide(false)` 便于取证。

## 5. 键盘 QA（xdotool 真实按键，逐次截图）

### 5.1 Tab 顺序（`keyboard/tab-00..08`、`tops.png` 拼接图）

初始焦点由程序放在工具栏区域（状态行 `Focus: toolbar`），随后连续 Tab 8 次，焦点环按视觉顺序推进：

| 步数 | 焦点 |
|---|---|
| 1 | 浅色 Light |
| 2 | 深色 Dark |
| 3 | English |
| 4 | 中文 |
| 5 | Ready |
| 6 | Loading |
| 7 | Empty |
| 8 | Error |

（顺序与工具栏视觉顺序完全一致；继续 Tab 到第 13 步停在 `Sheet…`，见 5.5。）

**Shift+Tab 可回退**：从 Error 退到 Empty（`shifttab-1-back-to-English.png`，截到的是 Empty 有环）、
再退到 Loading（`shifttab-2-back-to-Dark.png`）；从首个按钮（浅色）Shift+Tab 回到工具栏区域本身
（`focusring-dark-shifttab.png`，环消失，说明工具栏容器也是一个焦点停留点）。

### 5.2 焦点环在 light / dark 下均可辨识

- 浅色：`fr-0-tab1-light.png`（钴蓝环包住 Light 按钮，与选中态的灰色填充同时可见、互不混淆）
- 深色：`focusring-dark-strip.png`（上：Tab 后钴蓝环包住 浅色 按钮；下：Shift+Tab 回区域后环消失）
- 深色 + Enter 后：`dark-enter-strip.png` 下排

### 5.3 Enter 执行焦点所在动作

- `enter-activates-loading-state.png`：焦点在 Loading 按钮时按 Enter，表格立即切换为骨架加载、
  状态行变 `Data state: Loading`。
- `enter-activates-dark-theme.png`（+ `dark-enter-strip.png`）：焦点在 Dark 按钮时按 Enter，
  整窗切到深色主题，按钮进入选中态，**焦点保持在 Dark**。
- `kb-1-enter-opens-sheet.png`：焦点在 `Sheet…` 按钮时按 Enter，Sheet 打开（键盘触发覆盖层）。
- 侧栏 Enter（`sidebar-4-enter.png`）：确认当前导航目标，选择不变（行为与点击一致）。

### 5.4 方向键

- **DataTable 行选择**（`row-away-strip.png`：4 联图）：点击第 3 行选中后，Down → 第 4 行、
  Down → 第 5 行、Up → 第 4 行。**注意**：`row-strip.png` 里曾出现「按方向键不动」的假象——
  原因是鼠标指针停留在行上，hover 高亮遮住了选中的行；把指针移开后（`row-9-pointer-away.png`）
  方向键行为正常。此假象不是缺陷。
- **Tree**（`tree-strip.png` / `tree-strip2.png` / `tree-strip3.png`）：
  Down 在节点间下移（podman → snapshot-sync-worker）；Right 展开 Container（隐藏的子节点出现）；
  Left 收起已展开的 Container（`tree-7-left-collapse-expanded.png` 与展开前对比，子节点消失）。
- **侧栏**（`sidebar-strip.png`）：点击侧栏后 Down / Down / Up 在 概览→进程→端口→进程 间移动，
  状态行全程 `Focus: sidebar-panel`（`sidebar-focusline.png`）。
- **表格列间 Tab**（`col-strip.png`、`col04-focus.png`）：焦点在表格内时 Tab 被绑定为「下一列」，
  4 次后表格**横向滚动**并把 端口 列滚进可视区（960 宽下的关键可达性证据），
  状态行仍为 `Focus: data-table` —— 印证「Tab 无法离开表格」这一组件限制。

### 5.5 Escape 与焦点恢复

| 序列 | 证据 | 结果 |
|---|---|---|
| Sheet 打开（鼠标点 Sheet…，此前焦点在 Light 按钮）→ Escape | `fr-0..2` + `fr-toolbar-strip.png` | Sheet 关闭，焦点环回到打开前持有焦点的 **Light 按钮** |
| Sheet 打开（启动后未按 Tab，焦点在工具栏区域）→ Tab x2（第 2 次落在 Close，见 `frseq-sheet-close-strip.png` 下排）→ Escape | `frseq-0..3` | Sheet 关闭，状态行 `Focus: toolbar`，焦点回到打开前的工具栏区域 |
| AlertDialog 打开 → Tab（环落在 Cancel，`alert-dialog-strip.png`）→ Escape | `alert-0..2` + `alert-toolbar-strip.png` | 对话框关闭，焦点环回到 Light 按钮 |
| 键盘全程：Tab x13 到 `Sheet…` → Enter 打开 → Escape | `kb-0/1/2` + `kb-sheet-trigger-restore.png` | 关闭后焦点环回到**触发按钮 `Sheet…` 本身**（上排：打开前；下排：关闭后） |

结论：**覆盖层关闭后焦点恢复到「打开覆盖层之前持有焦点的元素」**。鼠标点击触发按钮不会把焦点
交给该按钮（gpui-component 按钮点击不取焦），所以纯鼠标流程恢复到打开前的焦点；
键盘流程（Tab 到按钮 → Enter）则严格回到触发控件。

### 5.6 焦点指示行的可用性

状态行能区分 `sidebar-panel` / `toolbar` / `data-table` / `unnamed region#tab-N`，对区域级定位有效；
但**所有工具栏按钮、Sheet 的 Close 都是 `unnamed region#tab-0`**——gallery 没有给这些控件分配
tab_index，导致焦点行无法区分「焦点在哪个按钮」，本轮键盘判定只能依赖可见焦点环（截图佐证）。
见问题 #3。

> 复验后修正（已落实）：控件已分配 tab 顺序号，焦点行可读出控件 ID——修正后证据见
> `keyboard/tab-readout-postfix.png`（Tab 2 步后读数 `Focus: theme-dark`，且 Dark 按钮带可见焦点环）。

## 6. 960x640 溢出 / 滚动 / 内容可用性

证据：`combo-960x640-*`、`keyboard/overflow-0-initial.png`、`keyboard/overflow-1-scrolled-right.png`、
`keyboard/col-strip.png`。

- 工具栏：换行（英文 2 行 / 中文 2 行），无裁切、无遮挡。
- 表格：列总宽（240+430+80+80 + 边框）超过 960 可视宽，**表格自身横向滚动**，
  右侧 PID 列被裁是预期行为；按 Tab 走列（或拖动横向滚动条）可把 端口 列滚进可视区
  （`col-strip.png` 右图已出现 端口 列）。纵向 8 行在可视高度内，无整窗滚动。
- 侧栏：`w_56` 固定宽，5 个导航项完整可见。
- 树：固定高度区域，可见节点正常，SSH / Shell 需在树内滚动（树自带滚动，非整窗滚动）。
- 状态栏：完整可见，焦点/数据状态/主题-语言三段都在。
- 未发现任何元素把内容推出窗口、出现整窗滚动条或重叠。

## 7. 发现的问题（未改代码）

标注约定：**QA 观察事项**（#1、#5）指「复验时容易误判、需要提示后人的观察陷阱」，不是代码缺陷；
同一内容在 `docs/qa/a4-gallery/README.md` 中记为「已知问题」。

| # | 级别 | 问题 | 证据 |
|---|---|---|---|
| 1 | 中（QA 观察事项） | **README（实现者自证）宣称「DataTable 方向键行选择已实测」，但该行为对鼠标选中行不生效的描述会误导**：实际是 hover 高亮遮蔽了选中行，指针不移开时看起来「方向键无效」。复验确认方向键正常，但实现者首轮记录未提示这一观察陷阱，后人易误判为回归（不是缺陷，见 README「已知问题」同一条） | `keyboard/row-strip.png`（假象）vs `keyboard/row-away-strip.png`（移开指针后正常） |
| 2 | 低 | **Notification 在 Ready 态只有标题没有正文**：`state_copy(Ready)` 返回空文案，回退为 gallery 标题，通知内容读起来像「组件实验台」而不是一条有信息量的消息。gallery 是取证工具，但产品壳层（B4）若沿用该文案策略会出现同样问题 | `overlays/open-notification.png` |
| 3 | 低 | **gallery 控件未设置 tab_index，焦点行读数无法区分按钮**：工具栏 13 个按钮与 Sheet 的 Close 都显示 `unnamed region#tab-0`，键盘 QA 只能靠可见焦点环判定。建议 gallery 给按钮分配 tab_index（或焦点行打印控件 ID），否则回归验证成本高。**已修正**：按视觉顺序分配 tab 顺序号 1..=17，焦点行按编号反查控件 ID | 修正前：`keyboard/strips.png`（8 步 Tab 全部同名）；修正后：`keyboard/tab-readout-postfix.png`（读数 `Focus: theme-dark`） |
| 4 | 低 | **unsupported 态图标过小**：图标是一条很短的「—」，与分隔线几乎无法区分，深色下尤其弱；与 error（红 X）、permission-denied（琥珀 eye-off）相比辨识度明显偏低。DESIGN.md 既然要求「状态不只靠颜色区分」，该图标建议加宽或加说明性图形。**已修正**：`StateView` 给该态短横加 40px 圆形外框（中性色不变） | 修正后证据：`states/state-unsupported.png`（已重拍为带外框形态；本报告拍摄时的裸短横旧图已随之更新） |
| 5 | 信息（QA 观察事项） | **`--lang en` 启动的首次窗口曾出现中文界面**：QA 首次启动时（上一实例被 pkill 后立刻重启）截图显示 zh-CN 界面与 `焦点: data-table`，与启动参数不符；随后干净重启无法复现，判定为上一实例残留窗口被复用（X 窗口 ID 6291457 复用）。记录在此，避免误判为参数解析缺陷（`Lang::parse` 与 `Dict::of` 代码审查无误）。不是缺陷，属环境观察事项（见 README「已知问题」同一条） | 本报告不复现，仅有首张作废截图 |

## 8. 库版本相关已知限制（随依赖版本失效）

> 本节条目是**当前锁定版本的行为事实**，不是 Runquiry 的设计要求（设计要求见 `DESIGN.md`）。
> 以下条目全部随 `Cargo.lock` 中 gpui-component 固定 rev `91217366` 而成立；升级该依赖时
> 必须逐条复验，已失效的条目应删除，不得当作长期契约。

| 条目 | 行为 | 对 Runquiry 的应对 |
|---|---|---|
| Root 不代绘覆盖层 | `Root` 只协调覆盖层状态与焦点恢复，不主动绘制；需内容视图显式调用 `Root::render_sheet_layer` / `render_dialog_layer` / `render_notification_layer`，调用顺序即层级（Notification 最后绘制，置于最上层） | 在内容视图接线三个 render_*_layer |
| Root 重入 panic | 覆盖层回调（打开/关闭/动作）内同步修改 `Root` 自身状态会触发重入 panic，需要 `defer` 推迟 | 覆盖层回调一律 defer；QA 复验打开/关闭链路 |
| SidebarMenuItem 不进 Tab 序 | 侧栏项是可点击 div，不进入 Tab 序 | 应用层以 Action + 焦点区域补齐侧栏键盘路径 |
| DataTable 内 `Tab` = 下一列 | 表格自身 key context 把 `Tab` 绑定为「下一列」，从表格内 `Tab` 无法离开表格；`Escape` 在无选中时向上传播，仍可关闭覆盖层 | QA 单独验证并记录（见 5.4）；侧栏/工具栏经 Shift+Tab 或点击回退 |
| 无焦点元素时 `Tab` 不移动 | 窗口内无元素持有焦点时 `Tab` 不会落焦 | 启动时程序性把初始焦点放到工具栏区域 |
| TreeState 不暴露 `FocusHandle` | 树组件无法持有键盘焦点，焦点证据只能到区域级 | 焦点行按区域读数；键盘验证依赖可见焦点环 |
| Button 不暴露 `FocusHandle`（点击不取焦） | 鼠标点击按钮不转移焦点，触发控件信息只能来自键盘路径 | 覆盖层焦点恢复以「打开前持有焦点的元素」为基准（见 5.5） |

## 9. 结论

- 8 组合、5 状态、3 覆盖层、键盘路径（Tab 顺序 / Shift+Tab / Enter / 方向键 / Escape / 焦点恢复）、
  960x640 溢出检查全部实测通过，证据齐备。
- 无阻塞缺陷；2 个中低优先级 UX/可观察性建议 + 2 个低优先级记录项（见第 7 节），均不影响 A4 验收。
## 10. Phase 0 整改复验（2026-09-03，Batch 2）

针对 §7/§5 遗留与 Batch 2 Phase 0 门禁要求的整改与真实复验。证据在
`screenshots/phase0/`（新目录，均由真实启动 + `import -window 0x<id>` 抓取）。

### 10.1 长中文 Name 列省略与完整值恢复（Phase 0 要求 1）

| 检查点 | 结果 | 证据 |
|---|---|---|
| Name 列 overflow hidden + text ellipsis | 通过：三条长中文条目在列宽内省略截断，不与相邻列重叠 | `phase0/name-ellipsis-zh.png` |
| Name/Path 单元格 tooltip 完整值 | 通过：hover 第一行名称，tooltip 显示完整「数据库连接池维护守护进程（长名称示例）」 | `phase0/name-tooltip-full-value.png` |
| Sheet 与当前选中行绑定 | **部分验证**：无选中行时 Sheet 诚实显示「尚未选择行…」（不再固定展示第一行，见 `phase0/sheet-no-selection.png`）；行选中本身可正常工作（`phase0/row-selected-hover.png`，第二行钴蓝选中 + 焦点行 `data-table`）；但「选中行 → 打开 Sheet 显示该行完整值」的组合链路在本次 QA 后段因 WSLg 输入注入失效（见 10.3）未能录得，留待下次复验 |

### 10.2 rust-i18n 迁移后的文案复验（Batch 2 B4 关联）

- gallery 与产品壳层的字典已由手写 `Dict` 迁移到 rust-i18n（`locales/app.yml`，v2
  格式与 gpui-component 一致），无第二套运行时字典；两种语言键完整性由
  `locale.rs` 单元测试强制（en/zh-CN 键集合一致 + `t!` 命中而非回显键名）。
- 迁移后中文界面截图（`phase0/name-ellipsis-zh.png`）与迁移前 A4 截图文案一致，
  未发现缺键或回显键名的界面元素。

### 10.3 QA 环境备注（WSLg 输入注入间歇性失效）

本次复验后段，`xdotool` 键盘注入与工具栏按钮点击间歇性失效（同一构建上此前
成功过：主题/语言/工作区切换、行点击均有成功记录；X 层 `getwindowfocus` 返回
错误，焦点不在 X 侧）。产品窗口的交互验证（主题/语言/工作区切换、设置落盘、
重启恢复、Ctrl+R）在失效发生前已完成；失效后未重试成功的检查项已在上表标注，
不属于产品缺陷，复验时建议先人工点击窗口激活再注入输入。

### 10.4 产品壳层（runquiry-app，B4）真实启动证据

| 检查点 | 结果 | 证据 |
|---|---|---|
| 壳层布局（Sidebar/Toolbar/主数据区/详情区/StatusBar） | 通过：1280×800 下 65/35 布局，四个工作区入口齐全 | `phase0/product-shell-light-en.png` |
| 主题切换 | 通过：Dark 按钮激活后整窗深色，设置文件立即写入 `{"theme":"dark"}` | 会话内截图，见 qa 过程 |
| 语言切换 | 通过：整窗中文（刷新/浅色/深色/工作区/采集器尚未接入…），主题保持深色，设置写入 `"language":"zh-CN"` | `phase0/product-shell-dark-zh-ports.png` |
| 工作区切换 | 通过：点击端口工作区，侧栏与状态栏同步，设置写入 `"last_workspace":"ports"` | 同上 |
| 设置文件脱敏 | 通过：文件仅含 allowlist 字段（theme/language/last_workspace/window），无任何调查输入 | 会话过程 `cat` 记录 |
| 重启恢复 | 通过：重启后直接进入深色 + 中文 + 端口工作区 | `phase0/product-restart-restore.png` |
| 诚实空态 | 通过：主数据区「采集器尚未接入」+ Spinner（Loading 语义），详情区「未选择对象」，无演示数据 | `phase0/product-shell-*.png` |

### 10.5 已知限制（本次新增，随当前锁定 GPUI/GPUI-Component 版本成立）

| 条目 | 行为 | 对 Runquiry 的应对 |
|---|---|---|
| X11 窗口关闭链路不完整 | `on_window_should_close` 回调与 `App::on_window_closed` 在 WM_DELETE_WINDOW 后均不触发；窗口销毁但进程残留（gpui 非 macOS 本应 LastWindowClosed 自动 quit，实测未走完 `window.removed` 分支） | 设置落盘以「事件即写」为主（theme/language/last_workspace 实测可用）；窗口尺寸由壳层渲染帧跟踪、随任意设置变更持久化；QA 收尾用 `pkill` 清理进程。升级 gpui 后必须复验 |
| WSLg 输入注入间歇失效 | 见 10.3 | 环境观察事项，非产品缺陷 |
