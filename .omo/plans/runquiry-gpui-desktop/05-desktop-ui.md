# 模块 05：GPUI 桌面壳层与产品工作区

上级方案：[Runquiry 桌面化实施计划](../runquiry-gpui-desktop.md)

## 模块目标

建立 Runquiry 的设计系统、应用状态和四个产品工作区，并将平台能力、部分结果和安全操作映射为一致的原生桌面交互。

## 所有权与边界

允许写入：

- DESIGN.md
- crates/runquiry-ui/
- crates/runquiry-app/src/
- UI 测试、fixture 展示和视觉 QA 证据

不得写入：

- runquiry-core 的领域规则。
- runquiry-platform 的系统采集实现。
- 根 Cargo 配置、Cargo.lock 和平台打包配置。

UI 只依赖 core 契约。缺少平台能力时展示 CapabilityStatus，不在 UI 内添加 OS 判断或备用采集实现。

## TODOs

- [x] **A4 设计系统与组件实验台**
  - 依赖：A1。
  - 在 DESIGN.md 固定语义颜色、字体、密度、间距、圆角、层级、状态和动效 token。
  - 使用中性石墨/灰色表面、钴蓝交互强调色；语义绿/黄/红不得替代交互强调色。
  - 设计读数固定为 3/2/9，不加入装饰性持续动画。
  - 构建独立 component gallery，覆盖 Sidebar、DataTable、Tree、Sheet、AlertDialog、Notification、StatusBar。
  - 每个组件展示 loading、empty、error、unsupported、permission-denied。
  - 同时展示 en、zh-CN、浅色、深色、960×640、1280×800、长中文和长路径。
  - 验证：真实启动 gallery，完成截图和焦点/键盘检查。
  - 完成证据：DESIGN.md、截图矩阵、发现问题及修正结果。
  - 实施记录（2026-09-03）：
    - DESIGN.md 为唯一设计规范（3/2/9、石墨表面、钴蓝强调、语义色隔离、token、五状态规则、窗口/滚动/键盘/可访问性）；只写稳定语义，库版本相关限制已移至 docs/qa/a4-gallery/qa-report.md §8（注明随 gpui-component rev 91217366 失效）。
    - 设计系统：crates/runquiry-ui（theme.rs 集中 46 项 PALETTE 原始色值 + 浅/深 Runquiry 主题、state.rs 六态 DataState、state_view.rs RenderOnce 统一状态呈现、locale.rs 最小确定性 en/zh-CN 字典占位——rust-i18n 留待 B4）；裸色值经 grep 核验仅存在于 theme.rs。
    - gallery：crates/runquiry-app/examples/gallery/（独立入口，不进产品窗口与安装包；放在 examples/ 而非 src/ 的原因是复用 runquiry-app 已内联声明的 gpui-platform 与 gpui-component-assets 依赖，属模块 05 允许写入 crates/runquiry-app/src/ 意图范围内的交付形式注记）。覆盖 Sidebar/DataTable/Tree/Sheet/AlertDialog/Notification/应用级 StatusBar 组合 × 五状态 × 双语 × 双主题 × 双尺寸 + 长中文与无空格长路径数据；CLI 参数 --size/--theme/--lang/--state/--open 支持 QA 脚本化；焦点指示行经 tab_index 可区分具体控件。
    - 视觉与键盘 QA：真实 X11 启动（env -u WAYLAND_DISPLAY），import -window 0x&lt;id&gt; 截图（本环境 root 抓图不可用），xdotool 真实按键注入。证据：docs/qa/a4-gallery/（qa-report.md + screenshots/ 8 组合 + 6 状态 + 4 overlay + 94 张键盘/焦点/滚动截图，含修正后补拍的 state-unsupported 与控件焦点读数）。Tab 19 步顺序与视觉一致、Shift+Tab 回退、Enter 激活动作、Escape 关 Sheet/AlertDialog 且焦点恢复触发控件、960×640 无溢出（表格横向滚动可达全部列）。
    - 已发现并修正的问题（8 条，详见 qa-report.md）：Root 不代绘覆盖层需显式 render_*_layer、window.update 内开 overlay 需 defer、树节点索引 ID 改领域 ID、NavActivate 空操作移除、note 对比度不足去 opacity、unsupported 图标加外框、Ready 态通知无正文、焦点读数 unnamed region#tab-0（tab_index 修复）。
    - 已知限制（记录于 qa-report.md）：SidebarMenuItem 不进 Tab 序（gallery 以方向键 NavNext/NavPrev 补齐，选择即激活、无 Enter 绑定——Enter 在侧栏没有可执行语义，见 qa-report §5.4 与 gallery nav_actions 注释）、DataTable 内 Tab=下一列无法离开表格、TreeState/Button 不暴露 FocusHandle、hover 高亮遮蔽选中行的观察陷阱。Phase 0 整改（2026-09-03）：Name 列 ellipsis + tooltip 完整值 + Sheet 绑定选中行，证据见 qa-report §10。
    - 验证结果：cargo fmt -p runquiry-ui -p runquiry-app 通过；cargo test -p runquiry-ui 10 个测试全过；cargo check -p runquiry-app --locked --examples 通过；clippy -D warnings 在基线豁免（-A clippy::multiple-crate-versions，app 另加 -A clippy::print-stderr）后零警告——两条原始失败均为基线既有问题（锁定依赖树 77 条 multiple-crate-versions，经还原 runquiry-ui/src 至骨架实测复现，与本次改动无关；print_stderr 为 A1 main.rs 既有例外），彻底消除需 A1 负责人调整根 lint 基线或依赖去重，超出本批次边界。
    - 未做（按边界属 B4/B5）：rust-i18n、设置持久化、刷新状态机/generation、业务数据绑定、Ctrl/Cmd+K/R/1..4 产品快捷键、100k 行虚拟滚动验证。

- [x] **B4 应用状态、刷新、设置与国际化**
  - 依赖：A3、A4。
  - 所有权分工（2026-09-03 修正）：runquiry-core 只承载纯刷新策略、generation 与不变量（不依赖 GPUI、计时器或 OS）；runquiry-ui 承载 GPUI Entity、Task、定时调度、每工作区 UI 状态与渲染；runquiry-app 承载 Root 装配、设置文件路径与持久化 I/O。
  - 初始化 gpui-component 并以 Root 装配应用壳层。
  - 建立固定侧栏、工具栏、主数据区、详情区和状态栏。
  - 为每个工作区维护独立 LoadState、generation、选择、排序和筛选。
  - 实现上级方案规定的 3-30 秒自适应刷新和 500ms 详情防抖。
  - 同一工作区禁止重入；手工刷新复用同一执行通道。
  - 只持久化主题、语言、窗口尺寸、最后工作区和列布局。
  - 使用 rust-i18n，locale 切换后立即 notify 当前视图。
  - 验证：gpui::test 覆盖刷新状态机、过期结果、设置恢复、主题和语言切换。
  - 完成证据：状态图、测试结果、设置文件样例必须不含调查数据。
  - 实施记录（2026-09-03，Batch 2；gpui test-support 会引入 deny 白名单外 git 源 proptest，故纯逻辑测试全部为普通 #[test]，GPUI 交互走真实 QA）：
    - core：`src/refresh.rs` 纯刷新策略——`Generation`（代际，is_stale 按值相等）与 `RefreshGate`（try_begin/finish/abort 重入门控 + witr `adjustRefreshInterval` 语义：3–30s、步长 3s、连续两次 >60% 退避、<30% 加速、中间区间计数归零），6 个单元测试覆盖阈值/连续计数/钳制/重入。
    - ui：`session.rs`（AppSession/WorkspaceSession：四工作区隔离的 LoadState/generation/选择/排序/筛选，手工与自动刷新共用 try_refresh 一条通道）、`debounce.rs`（500ms 详情防抖，注入时钟）、`shell/`（AppShell 产品壳层：侧栏四工作区/工具栏/主数据区/详情区/StatusBar，65/35 配比，ShellEvent 通知装配层）、`locale.rs`（rust-i18n 迁移：`i18n!("locales", fallback="en")` + `extend!(gpui_component)` 一次 + `set_language` 后显式 notify，删除手写 Dict，gallery 经 `tr()` 取文案）。ui 18 个测试。
    - app：`main.rs` 装配（Root 第一级视图、启动设置 ShellStartup 一次到位、Ctrl+R 绑定、订阅 ShellEvent 落盘）+ `settings.rs`（allowlist 白名单 serde schema，deny_unknown_fields；可注入路径；临时文件+rename 原子写；损坏回退默认不 panic；列布局 schema 先行、B5/B6 回填）。app 5 个测试。
    - 设置脱敏：结构上不存在调查输入字段（无 target/PID/路径/筛选/选择/调查结果），`deny_unknown_fields` 拒绝未知字段；真实运行验证设置文件仅含 `{"theme":"dark","language":"zh-CN","last_workspace":"ports"}`。
    - 真实 QA：壳层布局/主题/语言/工作区切换/重启恢复/Ctrl+R 全部实测通过（截图 docs/qa/a4-gallery/screenshots/phase0/）；诚实空态（采集器尚未接入）无演示数据。
    - 快捷键归属（模块验收「布局与交互验收」的剩余项）：Ctrl+R 已在 B4 落地；Ctrl+1..4（工作区切换，壳层范围）与 Ctrl/Cmd+K（调查入口）待 B5 随调查面板落地；方向键/Enter/Escape 已由组件库行为覆盖（gallery QA 验证）。
    - 已知限制（随当前锁定 gpui rev f66ed399 成立，升级后必须复验）：X11 窗口关闭链路不完整——on_window_should_close 与 App::on_window_closed 均不触发，窗口销毁后进程残留（gpui 非 macOS 本应 LastWindowClosed 自动 quit）；窗口尺寸以渲染帧跟踪、随任意设置变更持久化，QA 收尾 pkill 清理。WSLg 输入注入间歇性失效属环境观察事项。

- [ ] **B5 Processes 与调查工作区**
  - 依赖：B1、B2、B4。
  - 实现进程虚拟表格、显式目标类型调查栏、候选结果和详情面板。
  - 详情覆盖概览、祖先树、来源证据、告警、资源、Socket、文件、环境变量和操作入口。
  - 第一次 CPU 样本显示采样中；有第二个有效样本后显示数值。
  - 选择以稳定 ProcessIdentity 保存；目标消失时显示 stale 状态，不跳到其他行。
  - 敏感环境变量和命令参数默认脱敏；揭示仅在当前详情会话有效。
  - 验证：五类目标、多结果、无结果、PID 复用、进程消失、长数据和键盘流程。
  - 完成证据：GPUI 测试、真实 Linux 截图、脱敏检查。

- [ ] **B6 Ports、Containers、File Locks 工作区**
  - 依赖：B2、B3、B4。
  - Ports 支持仅监听/全部 Socket，显示协议、地址、端口、状态、PID、进程和公开监听。
  - Containers 显示运行时、名称、ID、状态、健康、镜像、主机 PID、启动时间。
  - File Locks 支持仅锁/全部打开文件，同一 PID/path 的锁记录优先。
  - 三页面均支持排序、筛选、稳定选择、详情和 generation。
  - 每个页面分别实现成功、空、部分、权限失败、工具失败和 Unsupported。
  - 验证：GPUI 交互测试和真实 Linux 数据流程。
  - 完成证据：各页面状态矩阵、测试结果、截图。

- [ ] **C3 能力矩阵与条件 UI**
  - 依赖：C1、C2。
  - 根据 PlatformCapabilities 生成页面和操作状态，不读取操作系统名称进行判断。
  - Linux/macOS 显示进程操作；Windows 不渲染可执行动作。
  - Windows File Locks 保留导航入口并解释系统能力限制。
  - Partial 状态显示已获得的数据和对应 issue；Unsupported 不使用错误 toast。
  - 验证：使用三平台假后端运行相同 UI contract tests。
  - 完成证据：平台 UI 矩阵、三套假后端测试结果。

## 布局与交互验收

- 默认窗口 1280×800，最小 960×640。
- 1100px 及以上为 65/35 可调整列表-详情布局；更窄时详情使用 Sheet。
- 侧栏、工具栏、状态栏固定；主表与详情分别拥有滚动区域。
- 表格行高 34px，100k 行时只渲染可见范围。
- Ctrl/Cmd+K、Ctrl/Cmd+R、Ctrl/Cmd+1..4、方向键、Enter、Escape 全部可用。
- AlertDialog 关闭后焦点返回触发控件。
- 主题和语言切换不会丢失当前工作区、筛选或选择。

## 模块退出条件

- 五项任务均完成并具有自动测试和真实截图证据。
- UI 没有直接系统调用、OS 分支或第二套领域判断。
- 中英文、浅深主题、窄/宽窗口和全状态矩阵均已验证。

## 交接格式

- 页面与状态矩阵。
- 快捷键和焦点路径。
- GPUI 测试命令与结果。
- 截图/录屏证据和剩余视觉风险。
