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

- [ ] **A4 设计系统与组件实验台**
  - 依赖：A1。
  - 在 DESIGN.md 固定语义颜色、字体、密度、间距、圆角、层级、状态和动效 token。
  - 使用中性石墨/灰色表面、钴蓝交互强调色；语义绿/黄/红不得替代交互强调色。
  - 设计读数固定为 3/2/9，不加入装饰性持续动画。
  - 构建独立 component gallery，覆盖 Sidebar、DataTable、Tree、Sheet、AlertDialog、Notification、StatusBar。
  - 每个组件展示 loading、empty、error、unsupported、permission-denied。
  - 同时展示 en、zh-CN、浅色、深色、960×640、1280×800、长中文和长路径。
  - 验证：真实启动 gallery，完成截图和焦点/键盘检查。
  - 完成证据：DESIGN.md、截图矩阵、发现问题及修正结果。

- [ ] **B4 应用状态、刷新、设置与国际化**
  - 依赖：A3、A4。
  - 初始化 gpui-component 并以 Root 装配应用壳层。
  - 建立固定侧栏、工具栏、主数据区、详情区和状态栏。
  - 为每个工作区维护独立 LoadState、generation、选择、排序和筛选。
  - 实现上级方案规定的 3-30 秒自适应刷新和 500ms 详情防抖。
  - 同一工作区禁止重入；手工刷新复用同一执行通道。
  - 只持久化主题、语言、窗口尺寸、最后工作区和列布局。
  - 使用 rust-i18n，locale 切换后立即 notify 当前视图。
  - 验证：gpui::test 覆盖刷新状态机、过期结果、设置恢复、主题和语言切换。
  - 完成证据：状态图、测试结果、设置文件样例必须不含调查数据。

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
