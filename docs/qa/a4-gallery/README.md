# A4 组件实验台 QA 证据

任务：`A4 设计系统与组件实验台`（[模块 05](../../../.omo/plans/runquiry-gpui-desktop/05-desktop-ui.md)）。
本文记录 gallery 的真实启动证据、键盘/焦点检查结果、发现问题与修正结果。

- 设计规范：仓库根 [DESIGN.md](../../../DESIGN.md)
- 设计系统实现：`crates/runquiry-ui/src/{theme,state,state_view,locale}.rs`
- 实验台入口：`crates/runquiry-app/examples/gallery/`（独立入口，examples 不参与发布；
  `main.rs` 保留视图状态与窗口装配，`table.rs` / `overlays.rs` / `cli.rs` / `data.rs`
  分别承载数据表、覆盖层、CLI 与合成数据）

## 1. 启动方式

```bash
cargo build --example gallery --locked
env -u WAYLAND_DISPLAY ./target/debug/examples/gallery [参数]
```

完整参数清单（QA 脚本可直接使用）：

| 参数 | 取值 | 默认 |
|---|---|---|
| `--size` | `960x640` \| `1280x800`（任意 `宽x高`，不得小于最小窗口） | `1280x800` |
| `--theme` | `light` \| `dark` | `light` |
| `--lang` | `en` \| `zh-CN` | `en` |
| `--state` | `ready` \| `loading` \| `empty` \| `error` \| `unsupported` \| `permission-denied` | `ready` |
| `--open` | `sheet` \| `alert` \| `notification` | 不打开 |

参数错误（未知参数、缺值、非法取值、小于最小窗口）→ stderr 提示 + 用法，退出码 2。
窗口标题固定为 `Runquiry Gallery`（`xdotool search --name "Runquiry Gallery"` 定位）。

## 2. 自动验证

| 命令 | 退出码 | 结果 |
|---|---|---|
| `cargo fmt -p runquiry-ui -p runquiry-app -- --check` | 0 | 通过 |
| `cargo test -p runquiry-ui --locked` | 0 | 10 个纯逻辑测试全部通过（主题 token 一致性、双语字典完整性、状态→图标/文案映射） |
| `cargo check -p runquiry-app --locked --examples` | 0 | 通过 |
| `cargo clippy -p runquiry-ui --all-targets -- -D warnings` | **101** | **基线既有问题**：77 条 `clippy::multiple-crate-versions`（锁定依赖树内重复版本）。已验证与本次改动无关：把 `crates/runquiry-ui/src` 还原到基线（仅占位 `lib.rs`）后同样报 77 条。加 `-A clippy::multiple-crate-versions` 后退出码 0 |
| `cargo clippy -p runquiry-app --all-targets -- -D warnings` | **101** | 同上 77 条 + `clippy::print_stderr`（`src/main.rs` 的既有 `eprintln!`，A1 遗留，`AGENTS.md` 已注明）。加 `-A clippy::multiple-crate-versions -A clippy::print-stderr` 后退出码 0 |

这两类 lint 只能由依赖守门人（根 `Cargo.toml` / `Cargo.lock`）与 `src/main.rs`（A1 代码）
处理，A4 无权修改；gallery 自身代码在上述豁免下零警告。

后续轮次（A4 修复）复验记录，结论一致：

| 命令 | 退出码 | 结果 |
|---|---|---|
| `cargo fmt -p runquiry-ui -p runquiry-app -- --check` | 0 | 通过 |
| `cargo test -p runquiry-ui --locked` | 0 | 10 个纯逻辑测试全部通过 |
| `cargo check -p runquiry-app --locked --examples` | 0 | 通过 |
| `cargo clippy -p runquiry-ui --all-targets --locked -- -D warnings -A clippy::multiple-crate-versions` | 0 | 通过 |
| `cargo clippy -p runquiry-app --all-targets --locked -- -D warnings -A clippy::multiple-crate-versions -A clippy::print-stderr` | 0 | 通过 |
| `cargo build --example gallery --locked` + `env -u WAYLAND_DISPLAY` 启动 | 0 | 窗口打开；Tab 19 步读数依次为 `toolbar`、`theme-light`…`open-sheet`、`data-table`，控件可区分；unsupported 态图标为带外框短横；Ready 态通知有正文；验证后 `pkill -x gallery` 无残留 |

### runquiry-ui 测试清单

- `theme::tests::palette_values_are_valid_rgba`：每个原始色值都能被 gpui 解析。
- `theme::tests::light_and_dark_cover_the_same_tokens`：浅/深主题覆盖同一 token 集合。
- `theme::tests::accent_is_not_a_surface_value`：钴蓝强调不与石墨表面同值。
- `state::tests::keys_are_unique_and_round_trip`、`parse_is_case_sensitive_and_rejects_unknown`。
- `state::tests::state_views_have_distinct_icons`：状态不只靠颜色区分。
- `locale::tests::lang_codes_round_trip`、`both_languages_are_complete`、`languages_differ`、
  `every_state_has_copy`。

## 3. 真实启动与截图矩阵

环境：WSLg，`env -u WAYLAND_DISPLAY` 强制 X11；`libEGL/MESA` 软件渲染警告属正常。
截图统一在 `screenshots/`（实现者首轮记录的 `shots/` 旧图已被独立复验集完全覆盖，
为避免两套证据漂移已删除）；均由 `import -window <Runquiry Gallery 窗口>` 抓取。

| 截图（`screenshots/` 下） | 参数 | 验证点 |
|---|---|---|
| `combo/combo-1280x800-light-zh-CN.png` | `--theme light --lang zh-CN` | 浅色石墨表面 + 钴蓝选中态、中文文案、长中文条目与无空格长路径（列内省略） |
| `combo/combo-1280x800-dark-en.png` | `--theme dark --lang en` | 深色主题、英文文案 |
| `combo/combo-960x640-dark-zh-CN.png` | `--size 960x640 --lang zh-CN --theme dark` | 最小窗口：工具栏换行不裁切，表格拥有自己的横向滚动 |
| `states/state-loading.png`（另见 `state-loading-dark-zh.png`） | `--state loading` | DataTable 内建骨架加载视图；树区显示不定进度指示 + 中文说明 |
| `states/state-empty.png` | `--state empty` | 空态（`inbox` 图标，中性色） |
| `states/state-error.png` | `--state error` | 错误态（`circle-x` 图标，`danger`） |
| `states/state-unsupported.png` | `--state unsupported` | 能力边界（圆形外框包住的短横，中性色，非错误样式，与分隔线可区分） |
| `states/state-permission-denied.png` | `--state permission-denied` | 权限边界（`eye-off` 图标，`warning`） |
| `overlays/open-sheet.png`（Escape 关闭证据见 `keyboard/fr-toolbar-strip.png`） | `--open sheet` | Sheet 右侧滑入；正常态显示详情摘要；Escape 关闭且焦点恢复 |
| `overlays/open-alert.png`（Escape 关闭证据见 `keyboard/alert-2-escape.png`） | `--open alert` | 确认对话框：命名对象 + 后果，Cancel + danger `Terminate`；Escape 关闭 |
| `overlays/open-notification.png` | `--open notification` | 通知（不自动隐藏，便于取证） |

其余 8 个基础组合（2 尺寸 × 2 主题 × 2 语言）见 `screenshots/combo/`；
键盘/焦点/滚动全过程见 `screenshots/keyboard/`。

### 启动日志

```
libEGL warning: DRI3 error: Could not get DRI3 device
libEGL warning: Ensure your X server supports DRI3 to get accelerated rendering
```

无 panic、无错误输出。

## 4. 键盘与焦点检查（xdotool 实测）

| 检查 | 操作 | 结果 |
|---|---|---|
| 初始焦点 | 启动 | 状态行显示 `焦点: toolbar` / `Focus: toolbar`（焦点落在工具栏，Tab 从这里开始） |
| Tab 顺序 | `Tab` | 焦点进入首个按钮，按钮出现可见的钴蓝焦点环（浅色/深色下都可辨识） |
| Enter 激活 | `Enter` | 激活聚焦的按钮：实测 `Dark` 按钮被激活，主题立即切换，选择不丢失。侧栏**没有** Enter 绑定（见第 6 节第 1 条） |
| 侧栏方向键 | 点击侧栏后 `Up`/`Down` | 选择在概览/进程间移动（`GallerySidebar` key context + `NavPrev`/`NavNext`）；选择即激活，Enter 无语义 |
| Escape 关闭覆盖层 | Sheet 打开后 `Escape` | Sheet 关闭，焦点回到触发前的 `toolbar`（`open-sheet-escape.png`） |
| 焦点实时显示 | 任意焦点变化 | 窗口底部状态行即时更新：区域（`sidebar-panel` / `toolbar` / `data-table` / `content-panel`）或具体控件 ID（`theme-light`、`ready`、`sheet-close` …），未命名的库内区域显示 `unnamed region#tab-N`。控件 ID 读数的修正后证据见 `screenshots/keyboard/tab-readout-postfix.png`（Tab 2 步后 `Focus: theme-dark`） |

## 5. 发现的问题与修正结果

| # | 问题 | 修正 |
|---|---|---|
| 1 | `--open sheet/alert/notification` 启动即 panic：`cannot update Root while it is already being updated`（在 `window.update` 内再次更新 Root） | 改为 `window.defer(cx, ...)`，推迟到本次 update 结束后打开；已重测，三种覆盖层均正常打开 |
| 2 | Sheet/AlertDialog/Notification 完全不显示 | 根因：当前 gpui-component（rev 9121736）的 `Root` 只协调覆盖层状态与焦点恢复，**不主动绘制**；需内容视图调用 `Root::render_sheet_layer` / `render_dialog_layer` / `render_notification_layer`。已在 gallery 视图补上（注释说明），B4 装配应用壳层时必须照做 |
| 3 | 正常态打开 Sheet 时出现无标题的加载指示（`StateView` 不应用于 `Ready`） | Sheet 在 `Ready` 改为详情摘要（合成数据），非正常态才用 `StateView` |
| 4 | Tab 在无初始焦点时不动（gpui `focus_next` 需要当前焦点） | 启动时聚焦工具栏区域，Tab 顺序由此开始 |
| 5 | 中文工具栏在 1280/960 宽度下最右按钮被裁切 | 覆盖层组允许换行（`flex_wrap` + `min_w_0`），窄窗口下整组换到第二行，不再裁切 |
| 6 | 树层级不可见 | 补每层 16px 缩进（结构深度的几何，注释说明为何用 px） |
| 7 | 通知默认自动隐藏，截图取证困难 | gallery 的通知设为 `autohide(false)` |
| 8 | 状态 note 文字叠加 `opacity(0.8)` 后对比度约 3.3:1，低于 4.5:1 | 去掉透明度：`muted_foreground` 本身即次级层级（约 4.8:1 达标） |
| 9 | unsupported 态图标是裸短横，与分隔线难以区分（深色下更差） | `StateView` 给该态图标加圆形外框（仍中性色、非错误语义），五个状态在形状上可区分 |
| 10 | 树节点用列表下标作元素 ID（`ListItem::new(ix)`），展开/收起时身份漂移 | 改用树节点的稳定领域 ID `entry.item().id`，与表格行的领域 ID 做法一致 |
| 11 | 侧栏的 `NavActivate`（Enter）只做 `cx.notify()`，是伪装成动作的空操作 | 删除该动作、`on_action` 与对应 `enter` 键绑定：方向键已把选择落到目标上，Enter 在 gallery 里没有可执行语义 |
| 12 | 工具栏按钮无 `tab_index`，状态行全部读作 `unnamed region#tab-0`，键盘 QA 无法判定焦点在哪个控件 | 按视觉顺序给工具栏 13 个按钮与 Sheet 的 Close 分配 tab 顺序号（1..=14），状态行按编号反查并显示控件 ID；区域容器编号排在其后，Tab 遍历顺序不变 |
| 13 | Notification 在 Ready 态没有正文（`state_copy(Ready)` 为空，回退成 gallery 标题） | 新增字典键 `notice_ready_body`（双语），Ready 态通知以状态短名作标题、以合成样本摘要作正文 |

## 6. 已知限制（移交 QA / B4）

1. **`SidebarMenuItem` 不进 Tab 序**：当前版本的侧栏项是可点击 `div`，没有 `track_focus`。
   gallery 以 `GallerySidebar` key context + `NavPrev`/`NavNext`（`Up`/`Down`）补齐键盘路径并
   已实测；**没有绑定 Enter**——gallery 里侧栏选择即激活（方向键与点击更新同一个 `nav`
   字段），Enter 没有可执行语义，绑定后只是空操作。B4 的真实侧栏若需要 Enter 确认
   （例如切换工作区），在该语义存在时再绑定。B4 若需要 Tab 直达侧栏，需上游组件支持
   或应用层自建焦点容器。
2. **`DataTable` 在自身 key context 内把 `Tab` 绑定为「下一列」**：焦点进入表格后 `Tab`
   被表格消费，不能用来离开表格（组件行为）。`Escape` 在无选中时会向上传播，因此仍能
   关闭覆盖层。
3. **`TreeState` 不暴露 `FocusHandle`**（gpui-base 私有字段），焦点状态行无法命名树的
   内部焦点，显示为 `unnamed region#tab-N`。侧栏/工具栏/表格（`TableState: Focusable`）
   有可读名称。
4. 库内 `Button` 等组件不暴露 `FocusHandle`。gallery 为工具栏按钮与 Sheet 的 Close 显式
   分配 tab 顺序号（1..=14），状态行据此显示控件 ID；区域容器的编号（15..=17）排在全部
   控件之后，否则 Tab 会先跳进侧栏/表格。仍未命名的库内区域（树、Sheet 内部容器等）
   显示 `unnamed region#tab-N`。
5. 侧栏菜单项的元素 ID 由组件按「父 ID + 序号」生成（组件内部行为）；gallery 的选择身份
   用 `Nav` 枚举值保存，语言切换/换序不受影响。
6. 表格行高为组件默认值（medium 密度）；产品要求的 34px 紧凑行高在 B5 数据绑定阶段按
   `DESIGN.md` §4 调整，gallery 只验证密度读数下的整体观感。
7. **hover 高亮陷阱（观察假象，非回归）**：指针停在表格行上时，该行的 hover 高亮会盖住
   选中行的高亮，此时按方向键「看起来没有效果」；把指针移开窗口（或移到非行区域）后
   再按方向键，行为正常。方向键选择本身没有缺陷——复验证据见 QA 报告的
   `row-strip.png`（假象）与 `row-away-strip.png`（移开指针后正常）。键盘 QA 时应先把
   指针移出表格，避免误判。
8. **QA 脚本启动前必须等待旧窗口销毁**：`pkill -x gallery` 后立刻重启，上一实例的 X 窗口
   尚未销毁，`xdotool search` 可能仍返回旧窗口 ID（实测出现过窗口 ID 6291457 被复用），
   导致截图与启动参数不符（QA 报告第 5 节问题 5 曾因此把 zh-CN 界面误记在 `--lang en`
   用例下）。脚本应在 `pkill` 后等待 `xdotool search --name "Runquiry Gallery"` 返回空再
   启动，并在截图前校验窗口 ID/标题与新实例对应。