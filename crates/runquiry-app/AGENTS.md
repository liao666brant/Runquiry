# Runquiry App 装配与入口

[crates](../) / runquiry-app

## 模块职责

可执行 crate（二进制名 `runquiry`）：依赖装配、窗口启动、配置持久化、资源与打包元数据。Batch 2 B4 起为真实产品壳层装配：runquiry-ui 的 `AppShell` 经 `Root` 装配进主窗口，设置持久化（allowlist 白名单）与快捷键在此层接线；Batch 4A I1 在本层装配 Linux 只读采集后端，UI 不直接访问操作系统或容器 CLI。

## 入口与启动

- 入口：`src/main.rs` 的 `main()`。
- 流程：`gpui_platform::application().with_assets(Assets)` → `runquiry_ui::locale::extend_component_translations()`（须在 gpui_component::init 之前，进程内一次）→ `gpui_component::init(cx)` → 从可用的绝对配置路径加载设置 → 构造 `PlatformBackend`（不可用时明确注入 `UnavailableBackend`）→ `cx.open_window` 创建窗口，第一级视图必须是 `gpui_component::Root`，内容视图为 `AppShell::new(ShellStartup, backend, …)` → 订阅 ShellEvent 与 `observe_window_bounds` 落盘 → 打开失败走 `eprintln!` + `cx.quit()`（无 unwrap/expect）。
- 已知限制（随当前锁定 gpui rev f66ed399 成立）：X11 下 `xdotool windowclose` 后 `on_window_should_close`/`on_window_closed` 均不触发，进程残留需 QA 主动停止；窗口尺寸在 bounds 变化时已经独立落盘，不受该限制影响。升级 gpui 后必须复验关闭退出。
- 运行：`cargo run -p runquiry-app --locked`（Linux 需 A1 安装的系统依赖：pkg-config、fontconfig、xkbcommon、wayland 等）。

## 对外接口

无对外 API。窗口行为（B4 起为产品壳层）：初始 1280×800（最小 960×640），`Root` 第一级视图装配 runquiry-ui 的 `AppShell`（侧栏四工作区/工具栏/主数据区/详情区/StatusBar）；启动设置（主题/语言/最后工作区/窗口尺寸）从设置文件恢复，`ShellEvent` 与窗口 bounds 变化写回 allowlist 设置。`PlatformBackend` 对每次 load/resolve/analyze 新建 `LinuxPlatform`，避免启动期进程排除基线漏掉后启动目标；共享 `AnalysisGate` 仍保证跨请求分析互斥。配置路径只接受平台环境提供的绝对路径；无安全路径时禁用持久化。

另有独立开发实验台：`src` 同级的 `examples/gallery/`（`cargo run -p runquiry-app --example gallery --locked`），A4 组件 gallery，不参与产品打包。

## 关键依赖与配置

- runquiry-core / runquiry-platform / runquiry-ui（workspace 继承）。
- **git 依赖内联声明**（不经 workspace 继承——cargo-deny 0.20 的 bans 无法解析 git 源的 workspace 继承依赖）：
  - `gpui`、`gpui_platform`（zed 仓库，rev 经 Cargo.lock 锁定 f66ed399）
  - `gpui-component`、`gpui-component-assets`（rev 91217366）
- 内联版本必须与根 `Cargo.toml` 注释保持一致；禁止无差别 `cargo update`（会使 GPUI 漂移到 zed main 新提交），所有验证使用 `--locked`。

## 测试与质量

- `cargo test -p runquiry-app --locked`：21 个测试（settings 安全/持久化、窗口 bounds 更新、平台后端的真实 Linux 进程/端口/文件目标解析与容器 fallback）。
- 其他验证：`cargo check -p runquiry-app --locked --examples`、`cargo deny check`、Linux 实际开窗（Batch 2 已做主题/语言/工作区切换与重启恢复的真实 QA）。
- lint 基线由根 `Cargo.toml` 的 `[workspace.lints]` 统一约束（`print_stderr` 为 warn，main 中的错误输出是当前唯一例外）。

## 常见问题

- WSLg 下 libEGL/MESA 软件渲染回退警告属正常现象，窗口功能不受影响。

## 相关文件清单

- `crates/runquiry-app/Cargo.toml` — manifest（含 git 依赖内联声明的原因注释）
- `crates/runquiry-app/src/main.rs` — 应用入口与最小窗口
- `crates/runquiry-app/src/backend.rs` / `src/backend/` — Linux 平台后端装配、fresh platform、共享分析门控、容器 fallback 与真实 Linux 回归
- `crates/runquiry-app/src/settings.rs` — allowlist 设置 schema、安全路径与原子写入
- `crates/runquiry-app/examples/gallery/` — A4 组件实验台（独立入口）
- `Cargo.toml` — 根 workspace 配置与 git 依赖锁定机制说明
- `Cargo.lock` — GPUI/gpui-component 提交锁定（勿手工编辑）
- `deny.toml` — cargo-deny 配置（许可证例外与允许的 git 来源）

## 变更记录

- 2026-09-02：初次索引。A1 最小窗口状态（gpui-component 初始化 + Root 第一级视图已验证）。
- 2026-09-03：新增 A4 gallery 示例（examples/gallery，独立入口不参与打包）。
- 2026-09-03：Batch 2 B4——重写 src/main.rs 为产品壳层装配；新增 src/settings.rs（设置持久化 allowlist，serde/serde_json 依赖经守门人批准，lock 内既有版本零新增包）。
- 2026-09-03：Batch 2 评审整改——「对外接口」段过期 `ShellView` 描述改写为产品壳层实际行为。
- 2026-09-03：Batch 2 评审修复——窗口 bounds 变化即时持久化并通过 1100×700 重启恢复；设置路径拒绝相对/共享临时回退；唯一排他临时文件阻断固定 `.tmp` 符号链接覆盖，Unix 新目录/文件为 0700/0600。
- 2026-09-04（未提交工作区）：Batch 4A I1——新增 `backend` 装配边界并将其注入 AppShell；每次采集/解析/分析建立 fresh `LinuxPlatform`，共享 `AnalysisGate` 保持跨请求互斥，端口无进程属主时可回退为已发布容器详情。快捷键改由 UI 的 `ProcessCommand` 统一映射；app 测试增至 21 个。
