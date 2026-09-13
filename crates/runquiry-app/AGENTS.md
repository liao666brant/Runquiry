# Runquiry App 装配与入口

[crates](../) / runquiry-app

## 模块职责

可执行 crate（二进制名 `runquiry`）：依赖装配、窗口启动、命令行元数据、配置持久化、资源与打包元数据。`src/main.rs` 完成壳层与平台后端的装配与设置接线，`src/backend.rs` + `src/backend/` 把 core 端口实现接到 UI 的 `WorkspaceBackend` 边界（含 fresh platform、分析互斥、CPU% 两样本差分与容器目标回退），`src/settings.rs` 负责 allowlist 设置 schema 与安全落盘。UI 不直接访问操作系统或容器 CLI，平台门禁（非 Linux/Windows 拒绝）在 runquiry-platform。

## 入口与启动

- 入口：`src/main.rs` 的 `main()`；`#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` 使 release 不附带控制台窗口（debug 保留控制台输出）。
- 命令行元数据：`--version`/`-V` 打印 `version_line()`（`CARGO_PKG_VERSION` + `consts::OS/ARCH` + profile），`--help`/`-h` 打印 `HELP_TEXT`，均以退出码 0 返回；未知参数 `eprintln!` 后 `std::process::exit(2)`。全部由标准库实现，不读取外部文件。
- 启动流程：加载默认设置路径 → `gpui_kit::application().with_assets(gpui_kit::assets::Assets)` → `set_app_identity` → `runquiry_ui::locale::extend_component_translations()`（须在 `gpui_kit::init` 之前）→ `gpui_kit::init(cx)` → `theme::install` + `cx.bind_keys(ProcessCommand::bindings())` + `activate(true)` → 写入全局 `SettingsStore` 并应用主题/语言。
- 窗口：尺寸取设置（默认 1280×800，最小 960×640，`clamped()`）并居中；`WindowOptions` 以 `TitleBar::window_options()` 为基底（原生标题栏透明 + 自绘窗口控制），`app_id = "runquiry"`，Linux 下 `icon` 由 `include_bytes!("assets/icons/runquiry.png")` 解码（失败仅 `eprintln!` 并留空）；内容视图 `AppShell::new(ShellStartup, backend, …)`，第一级视图必须是 `gpui_kit::component::Root`。
- 后端注入：`PlatformBackend::new()` 成功则 `Arc<PlatformBackend>`，失败则 `UnavailableBackend::new(error)`（能力报 `Unavailable`、动作返回 `Unsupported`）；打开窗口失败走 `eprintln!` + `cx.quit()`，全路径无 `unwrap`/`expect`。
- 设置接线：订阅 `ShellEvent`（主题/语言/工作区/隐藏列变化）落盘、`observe_window_bounds` 记忆窗口尺寸并落盘、初始焦点置壳层工具栏、`on_window_should_close` 落盘后退出、`on_window_closed` 兜底退出；持久化失败只 `eprintln!` 不中断。

## 对外接口

无对外 API。窗口行为（B4 起为产品壳层）即其外部契约，另有两项开发用入口：

- `src/backend.rs` 的 `PlatformBackend` 实现 UI 的 `WorkspaceBackend`：按 `target_os` 装配平台结构体（`LinuxPlatform`/`WindowsPlatform`），每次 load/resolve/analyze/动作/能力查询都新建 fresh platform（避免启动期进程排除基线漏掉后启动的目标），共享 `AnalysisGate` 保证跨请求分析互斥；`failed_collection_error` 把采集失败的诊断分流为 `Unsupported`（保留诊断原文）、`PermissionDenied`（保留 subject）或带首条诊断的 `Unsupported`；进程动作与「打开文件位置」分别转发 `ProcessController::execute` 与 `reveal_executable`，能力经 `action_capability`/`reveal_capability` 逐动作查询（构造失败时返回 `ProcessActionCapabilities::all_unavailable`，`process_control_capability` 返回 `Unavailable`）。
- `src/backend/cpu_sample.rs`：跨刷新保留 `pid → 上次累计 CPU 秒`，用两样本差分回写 `cpu_percent`；首样本、墙钟间隔非正、CPU 时间倒退（PID 复用）为 `None`，每轮整表替换基准；对 crate 内可见（`pub(super)`），4 个纯函数测试。
- 开发实验台：`examples/gallery/`（A4 组件实验台，CLI 驱动 `--size/--theme/--lang/--state/--open`，合成数据、不读本机、不参与打包）；`examples/scale_qa/`（B8 10 万行合成桌面 QA 入口，无副作用）。

## 关键依赖与配置

- 依赖：`runquiry-core` / `runquiry-platform` / `runquiry-ui`（workspace 继承）、`gpui-kit.workspace = true`、`serde 1.0.229`（derive）、`serde_json 1.0.151`；target 条件依赖 `image = "=0.25.10"`（仅 Linux，`png` feature）；build-dependencies `embed-resource = "=3.0.11"`。传递依赖由 `Cargo.lock` 锁定，禁止无差别 `cargo update`，所有验证与打包使用 `--locked`。
- 资源与图标：`assets/icons/`（`runquiry.ico` 供 `build.rs`/打包器，`runquiry-512.png` 供打包器，`runquiry.png` 供 Linux 运行时图标，`runquiry.desktop` 供 Linux 桌面入口），`assets/branding/runquiry-icon-concept-v1.png` 为原始概念图（无代码引用）。`build.rs` + `resources/runquiry.rc` 仅在 Windows 目标把资源 ID 1 的图标嵌入 `runquiry` 二进制（GPUI Windows 后端按该 ID 加载）。
- 打包：根 `Packager.toml`（cargo-packager，`formats = ["wix","deb","appimage"]`，`out_dir = "dist"`，无签名；deb 段含依赖列表与 `packaging/runquiry.desktop.hbs` 桌面模板，`[deb.files]`/`[appimage.files]` 安装第三方许可证与 NOTICE）；`about.toml` + `about.hbs` 生成 `docs/third-party-licenses.md`；`scripts/package-windows.sh` 为 Windows 打包 runner（`cargo about generate` → `--locked` release 构建 → MSI → 便携版 zip → `dist/SHA256SUMS` → 断言 Cargo.lock 未漂移）。当前只有 Windows 产物实测验证过。

## 数据模型

- 设置文件 `Settings`（`#[serde(deny_unknown_fields)]`）：`theme`、`language`、`window`、`last_workspace` 为可选字段；`column_layouts`（工作区 → 列 ID → 宽度）与 `hidden_columns`（工作区 → 隐藏列 ID 集合）为映射字段。全部字段带 `#[serde(default, skip_serializing_if = ...)]`，保证旧文件可读、旧版本可继续读取新文件。
- `WindowSize`：`DEFAULT` 1280×800、`MIN` 960×640，`clamped()` 保证读取值不越界。
- 路径与落盘：只接受平台环境提供的绝对路径（相对或缺失基路径即禁用持久化）；写入使用同目录唯一排他临时文件（`create_new`，PID + 原子序号，Unix 0600/目录 0700）→ `write_all` + `sync_all` → `rename`，失败清理临时文件。默认路径为 Windows `%APPDATA%\runquiry\settings.json` 与其它平台的 `$XDG_CONFIG_HOME`（缺省 `$HOME/.config`）下 `runquiry/settings.json`。

## 测试与质量

- `cargo test -p runquiry-app --locked`：静态计数 32 个 `#[test]` 在二进制目标内——`src/main.rs` 3、`src/settings.rs` 12（其中 2 个 `cfg(unix)`）、`src/backend/container.rs` 2、`src/backend/cpu_sample.rs` 4、`src/backend/tests.rs` 6（含按 `cfg!(target_os)` 分支的 reveal 能力断言）、`src/backend/tests/linux_only.rs` 5（整模块 `cfg(target_os = "linux")`，其中 2 个经环境变量早返回）。另有 `examples/scale_qa` 的 3 个 `#[test]`（经 `#[path]` 引入）。
- 其他验证：`cargo check -p runquiry-app --locked --all-targets`、`cargo deny check`、实际开窗与设置重启恢复（历史 QA 与新平台通道的证据见 `.omo/evidence/`）。
- lint 基线由根 `Cargo.toml` 的 `[workspace.lints]` 统一约束（`print_stderr` 为 warn，`main` 中的错误输出是当前唯一例外）。

## 常见问题

- WSLg 下 libEGL/MESA 软件渲染回退警告属正常现象，窗口功能不受影响。
- X11 下用 `xdotool windowclose`/`windowquit` 关闭窗口后进程仍可能存活（GPUI 与 GPUI Kit 均有该行为），需按 PID 主动停止；窗口尺寸仍由 bounds 变化独立落盘。
- 设置持久化依赖平台提供的绝对配置路径；路径缺失或非绝对时静默禁用持久化，不回落共享临时目录。

## 相关文件清单

- `crates/runquiry-app/src/main.rs` — 应用入口、命令行元数据、窗口装配、快捷键与设置接线
- `crates/runquiry-app/src/backend.rs` — `PlatformBackend`：平台选择、fresh platform、能力/动作/reveal 接线与错误映射
- `crates/runquiry-app/src/backend/analysis_gate.rs` — 跨 fresh platform 的分析互斥门
- `crates/runquiry-app/src/backend/container.rs` — 容器目标解析与宿主 PID 验证后的进程回退
- `crates/runquiry-app/src/backend/cpu_sample.rs` — CPU% 两样本差分状态
- `crates/runquiry-app/src/backend/unavailable.rs` — 平台构造失败时的 `Unavailable` 边界后端
- `crates/runquiry-app/src/backend/tests.rs`、`src/backend/tests/linux_only.rs` — 后端回归测试（含 Linux 实机目标解析）
- `crates/runquiry-app/src/settings.rs` — allowlist 设置 schema、安全路径与原子写入
- `crates/runquiry-app/Cargo.toml`、`build.rs`、`resources/runquiry.rc` — manifest、Windows 图标资源嵌入
- `crates/runquiry-app/examples/gallery/`、`examples/scale_qa/` — 组件实验台与 10 万行桌面 QA 入口
- `assets/icons/`、`assets/branding/` — 图标资源与原始品牌图
- `Packager.toml`、`about.toml`、`about.hbs`、`packaging/runquiry.desktop.hbs`、`scripts/package-windows.sh` — 打包与许可证清单配置
- `Cargo.toml`、`Cargo.lock`、`deny.toml` — 根 workspace 配置、依赖锁定与许可证/来源检查
