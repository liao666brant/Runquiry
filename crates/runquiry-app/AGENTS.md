# Runquiry App 装配与入口

[crates](../) / runquiry-app

## 模块职责

可执行 crate（二进制名 `runquiry`）：依赖装配、窗口启动、配置持久化、资源与打包元数据。当前仅包含 A1 阶段验证技术栈的最小窗口。

## 入口与启动

- 入口：`src/main.rs` 的 `main()`。
- 流程：`gpui_platform::application().with_assets(Assets)` → `gpui_component::init(cx)` → `cx.open_window` 创建窗口，第一级视图必须是 `gpui_component::Root`（gpui-component Root 契约）→ 设置标题 "Runquiry"；打开失败走 `eprintln!` + `cx.quit()`（无 unwrap/expect）。
- 运行：`cargo run -p runquiry-app --locked`（Linux 需 A1 安装的系统依赖：pkg-config、fontconfig、xkbcommon、wayland 等）。

## 对外接口

无对外 API。窗口行为：初始 1280×800（最小 960×640），居中显示 "Runquiry" 文本的壳层视图 `ShellView`（占位，产品工作区由 A4 实现）。

另有独立开发实验台：`src` 同级的 `examples/gallery/`（`cargo run -p runquiry-app --example gallery --locked`），A4 组件 gallery，不参与产品打包。

## 关键依赖与配置

- runquiry-core / runquiry-platform / runquiry-ui（workspace 继承）。
- **git 依赖内联声明**（不经 workspace 继承——cargo-deny 0.20 的 bans 无法解析 git 源的 workspace 继承依赖）：
  - `gpui`、`gpui_platform`（zed 仓库，rev 经 Cargo.lock 锁定 f66ed399）
  - `gpui-component`、`gpui-component-assets`（rev 91217366）
- 内联版本必须与根 `Cargo.toml` 注释保持一致；禁止无差别 `cargo update`（会使 GPUI 漂移到 zed main 新提交），所有验证使用 `--locked`。

## 测试与质量

- 测试未发现。当前验证手段：`cargo check -p runquiry-app --locked`、`cargo deny check`、Linux 实际开窗（A1 已验证）。
- lint 基线由根 `Cargo.toml` 的 `[workspace.lints]` 统一约束（`print_stderr` 为 warn，main 中的错误输出是当前唯一例外）。

## 常见问题

- WSLg 下 libEGL/MESA 软件渲染回退警告属正常现象，窗口功能不受影响。

## 相关文件清单

- `crates/runquiry-app/Cargo.toml` — manifest（含 git 依赖内联声明的原因注释）
- `crates/runquiry-app/src/main.rs` — 应用入口与最小窗口
- `crates/runquiry-app/examples/gallery/` — A4 组件实验台（独立入口）
- `Cargo.toml` — 根 workspace 配置与 git 依赖锁定机制说明
- `Cargo.lock` — GPUI/gpui-component 提交锁定（勿手工编辑）
- `deny.toml` — cargo-deny 配置（许可证例外与允许的 git 来源）

## 变更记录

- 2026-09-02：初次索引。A1 最小窗口状态（gpui-component 初始化 + Root 第一级视图已验证）。
- 2026-09-03：新增 A4 gallery 示例（examples/gallery，独立入口不参与打包）。