# Batch 4A 输入 placeholder 语言切换修复证据

## 代码修复

- `AppShell::set_language` 现在接收当前窗口的 `&mut Window`，在全局 locale 切换后，使用当前 locale 的 `t!("query.placeholder")` 与 `t!("filter.placeholder")` 更新两个既有 `InputState`。
- 仅修改两个语言按钮 listener 的参数传递；没有重建输入实体，因此 query/filter 值、焦点、工作区选择、表格选择和宽窗 `Resizable` 状态保持不变。

## 自动化门禁

| 场景 | 精确调用 | 可观察结果 | 证据 |
|---|---|---|---|
| UI 单元/纯逻辑测试 | `cargo test -p runquiry-ui --locked` | `48 passed; 0 failed` | 本次命令退出码 0；终端输出 |
| 应用编译检查 | `cargo check -p runquiry-app --locked` | `runquiry-ui` 与 `runquiry-app` 检查完成，退出码 0 | 本次命令终端输出 |
| 锁定应用构建 | `cargo build -p runquiry-app --locked` | 当前二进制生成成功，退出码 0 | `target/debug/runquiry`，SHA-256 `b085bfa101fc617ace1f1126487af9458b6d2268efb636320a22e400fbdaedc3` |
| 严格 Clippy | `cargo clippy -p runquiry-ui -p runquiry-app --all-targets --locked -- -D warnings -A clippy::multiple_crate_versions -A clippy::print_stderr` | 退出码 0；仅显式保留项目既有两项例外 | 本次命令终端输出 |
| 格式/差异 | `cargo fmt --all -- --check`；`git diff --check -- crates/runquiry-ui/src/shell/mod.rs crates/runquiry-ui/src/shell/render.rs` | 两命令退出码 0 | 本次命令终端输出 |

## 真实运行 QA

本子代理尝试执行：

```text
env DISPLAY=:0 WAYLAND_DISPLAY= XDG_CONFIG_HOME=/tmp/runquiry-locale-qa-wsW6Gd target/debug/runquiry
```

当前子代理 shell 没有可用 WSLg X11 display；`DISPLAY=:0 xdpyinfo` 返回 `unable to open display ":0"`，应用启动报 `Failed to initialize X11 client`。因此本文件不宣称真实窗口语言切换通过；父代理必须在可用图形会话中执行下列场景，并把截图路径与 SHA-256 补充到本文件：

1. 启动当前 `target/debug/runquiry`，在 1280px 宽窗输入并保留非空 filter、查询值、工作区与行选择；点击 English -> 中文，确认值/选择/工作区/拖拽分隔位置不变。
2. 清空 query/filter，截图中文 query 与 filter placeholder，切回 English，再截图两个英文 placeholder。
3. 清理本轮专属应用 PID、窗口和 `/tmp/runquiry-locale-qa-wsW6Gd`；确认未触碰 `127.0.0.1:46245` 的 codex app-server。

## 代码规模

- `crates/runquiry-ui/src/shell/mod.rs`：189 行纯代码。
- `crates/runquiry-ui/src/shell/render.rs`：247 行纯代码，处于 200–250 警告带；本修复未新增模块或依赖。
