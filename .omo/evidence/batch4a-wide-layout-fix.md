# Batch 4A 宽窗详情布局修复证据

- 日期：2026-09-04（Asia/Shanghai）
- 范围：仅 `render_workspace.rs`、`render_detail.rs`、`render_workspace_detail.rs`；没有修改
  app/core/platform、根 Cargo 文件、`witr/`、计划或状态文件。
- 红证据：`docs/qa/batch4a/screenshots/targets-final-name-pid.png`、
  `targets-final-port.png`、`targets-final-file.png` 均为 1280×800 PNG；这些截图中的长详情将
  主区压缩为近似单字符列。源码审计确认主/详情原先只设置 `flex_grow(65/35)`，保留 auto basis。

## 最小修复

1. `render_workspace.rs`：主面板改为 `.flex_1().flex_grow(MAIN_RATIO)`，调查行及输入包裹
   加 `.min_w_0()`。
2. `render_detail.rs`：详情面板改为 `.flex_1().flex_grow(DETAIL_RATIO)`，保持既有 65/35
   常量；加 `.overflow_x_hidden()`，并为详情根、标题、动态值、环境项、issue 加
   `.min_w_0()` / `.whitespace_normal()`。
3. `render_workspace_detail.rs`：Port/File/Container 详情根与 issue 文本沿用相同的收缩和
   正常换行约束。

锁定 GPUI 源码的 `Styled::flex_1` 明确设置 `grow=1`、`shrink=1`、`basis=0`；随后调用
`flex_grow(65/35)` 仅覆盖 grow，因此两个同级面板以零基准按既有比例分配空间。

## 自动门禁

| 场景 | 调用 | 二进制可观察结果 | 捕获工件 |
| --- | --- | --- | --- |
| UI 回归 | `cargo test -p runquiry-ui --locked` | exit 0；47 passed、0 failed | 本文件的命令记录 |
| App 回归 | `cargo test -p runquiry-app --locked` | 非沙箱复验 exit 0；20 passed、0 failed。首次沙箱运行仅 `resolves_post_start_loopback_port_holder` 因 EPERM 失败，非沙箱同命令恢复真实回环权限后通过。 | 本文件的命令记录 |
| 编译 | `cargo check -p runquiry-app --locked` | exit 0 | 本文件的命令记录 |
| 严格 Clippy | `cargo clippy -p runquiry-ui -p runquiry-app --all-targets --locked -- -D warnings -A clippy::multiple-crate-versions -A clippy::print-stderr` | exit 0；仅使用仓库已有两项例外 | 本文件的命令记录 |
| 格式与空白 | `cargo fmt --check`；`git diff --check` | 两者 exit 0、无输出 | 本文件的命令记录 |
| 自有生产文件规模 | `awk` 统计非空、非纯注释行 | workspace=204、detail=249、workspace_detail=155，均不超过 250 | 本文件的命令记录 |

## 真实 WSLg QA（修复后新鲜工件）

构建调用为 `cargo build -p runquiry-app --locked`（exit 0）；随后以：

```text
setsid env DISPLAY=:0 WAYLAND_DISPLAY= XDG_CONFIG_HOME=/tmp/runquiry-wide-layout-retry-gOYdEO/config target/debug/runquiry
```

启动专属窗口。运行态可观察值为 Runquiry PID `187205`、X11 window `18874369`；
`DISPLAY=:0 xset q` 返回 `AVAILABLE`。所有以下 PNG 由：

```text
DISPLAY=:0 import -window 18874369 <artifact>
```

在最后一次 Rust 编辑和构建之后截取，并以 `file` / `identify` 复核为完整 RGB PNG 及所列尺寸。

| 成功准则与场景 | 精确调用 / 输入 | 二进制可观察结果 | 持久化工件 |
| --- | --- | --- | --- |
| 宽窗 Process | 1280×800；选择 PID；输入受控 `187999`（`runquiry-wide-layout-process`）并点击 Investigate | `sleep · PID 187999` 详情出现；主/详情分界约 x=903（扣 223px 侧栏后约 680/377，65/35）；表格四列完整可读 | `docs/qa/batch4a/screenshots/wide-layout-green-process-pid-1280.png` |
| 宽窗 Port | 1280×800；选择 Port；输入受控回环监听 `37191` 并点击 Investigate | `nc · PID 188000` 与 `Tcp 127.0.0.1:37191 · LISTEN` 出现在详情；主表未收缩为单字符列 | `docs/qa/batch4a/screenshots/wide-layout-green-port-1280.png` |
| 宽窗 File | 1280×800；选择 File；输入受控 flock 文件 `/tmp/runquiry-wide-layout-retry-gOYdEO/qa-lock.txt` 并点击 Investigate | `sleep · PID 188001` 详情出现；长临时路径、中英 issue 和 systemd 描述均在界面边界内 | `docs/qa/batch4a/screenshots/wide-layout-green-file-1280.png` |
| 紧凑 Sheet | 960×640；点击真实 systemd/PID 1 表行 | 右侧 Details Sheet 显示 `systemd · PID 1`；背景主表保持可扫描，详情未内联压缩主区 | `docs/qa/batch4a/screenshots/wide-layout-green-process-960-sheet.png` |
| 1099 断点 | 对同一窗口执行 `xdotool windowsize 18874369 1099 700`，先 Escape 关闭 Sheet | 无内联详情，主区占满可用工作区；与 `shows_inline_detail(1099) == false` 单测一致 | `docs/qa/batch4a/screenshots/wide-layout-green-process-1099-compact.png` |

截图 SHA-256：

```text
e8a913c1afebe96fa19377dd5acf21788db06dfc98b5a71eedb929c7a3617f0b  wide-layout-green-process-pid-1280.png
d88e0df91fb3deab5c66652696bf7ee1ed558a1606ebf868c3529cf7d2401f26  wide-layout-green-port-1280.png
0bf41b3e8e7fc50dc6090dbbf84c4c588dfd3ff5e57d32ecf667ed6f103b2192  wide-layout-green-file-1280.png
d543523993d38d44e1a1e261b274f967e0d5a8e903fc3290b7c181266d87b789  wide-layout-green-process-960-sheet.png
291d9aef9394ccb5040bf7323cdd58059d14e42b1c59f4fec9343b7f55a07e0a  wide-layout-green-process-1099-compact.png
```

旧红图与新图包含不同的实时进程、PID、路径与文本，不能作为像素相似度阈值；三组
`visual-qa.mjs image-diff` 均确认同尺寸和完整 alpha 通道，但得到 0/100 相似度，因此没有
将其作为通过依据。通过依据是上表的同一产品交互路径及新鲜实景截图。

## 独立视觉审查

视觉 QA 双独立只读审查均对同一组 5 张新鲜截图返回 `PASS` / high confidence、无 blocking：

- Design-system/functional 审查：确认真实 GPUI/gpui-component 树、零基准 65/35、详情边界
  溢出控制、960 Sheet 与 1099 compact 均成立。
- Visual/CJK 审查：确认 Process/Port/File 主表没有单字符列；长路径和中英 issue 未裁切、
  没有 CJK 孤字或 tofu；960 Sheet 与 1099 无内联详情均正确。

## 启动诊断与清理回执

第一次未使用 `setsid` 的后台启动（PID `185733`、`/tmp/runquiry-wide-layout-Z916j5`）在 shell
返回后已无进程/窗口且日志为空。以下可区分观测确认是后台会话生命周期，而非产品或 X11
启动失败：重试使用 `setsid` 后 PID `187205` 仍存活、`xset q` 为 AVAILABLE、窗口 `18874369`
可见且日志仅含预期 WSLg `libEGL` DRI3 软件渲染警告。

本轮只创建并只终止了 PID `187205`（Runquiry）、`187999`（sleep）、`188000`（nc）、
`188001`（flock/sleep）。清理调用：

```text
kill -TERM 187999 188000 188001 187205
rm -rf /tmp/runquiry-wide-layout-retry-gOYdEO /tmp/runquiry-wide-layout-Z916j5
```

清理后的可观察结果：这些 PID 与 PID 对应窗口均无输出；`ss -H -ltnp 'sport = :37191'` 无输出；
两个专属临时目录不存在。`127.0.0.1:46245` 始终仍由 `codex app-serve` PID `1421` 监听，未被
本轮读取以外的动作触碰。
