# Batch 4A 可调整分栏与长证据行修复

- 日期：2026-09-04（Asia/Shanghai）
- 范围：仅 `crates/runquiry-ui/src/shell/render.rs` 与
  `crates/runquiry-ui/src/shell/render_evidence.rs`，以及本报告和对应截图；未修改依赖、根
  Cargo 文件、平台/核心/应用层、计划或参考 `witr/`。
- 构建产物：`target/debug/runquiry` SHA-256
  `22ab2db801e9a62cf04947719478ff6f2e962337c6547312eee26aa679b4d2ca`。

## 修复

1. 宽度不低于 1100px 时，固定侧栏之外使用锁定 `gpui-component` rev `91217366` 的
   `h_resizable("workspace-detail-split")` 和两个 `resizable_panel()`。稳定元素 ID 让
   组件的 keyed `ResizableState` 在本次窗口会话重渲染后保留用户拖拽的分栏。
2. 初始主栏宽度以 `gpui::Pixels` 计算：扣除 224px 侧栏后按 65/35 分配；主/详情最小宽度
   分别为 360px/280px。没有调用面板保留样式中禁止的 `flex_basis`，运行时 basis 由
   `ResizableState` 管理。
3. 低于 1100px 时不构造 resizable group，保持既有完整主栏与 Sheet 路径。
4. 证据总组、分组标签、动态条目和空值都加了 `min_w_0().whitespace_normal()`；详情已有
   横向裁切仅作为边界防护，长路径由条目自身换行而不是被静默裁掉。

## 自动验证

| 场景 | 调用 | 可观察结果 | 工件 |
| --- | --- | --- | --- |
| UI 回归 | `cargo test -p runquiry-ui --locked` | 48 passed, 0 failed | 命令输出；新增 65/35 像素比例测试通过 |
| 应用回归 | `cargo test -p runquiry-app --locked` | 真实回环环境 21 passed, 0 failed | 命令输出 |
| 编译 | `cargo check -p runquiry-app --locked`；`cargo build -p runquiry-app --locked` | 两者 exit 0 | SHA-256 如上 |
| 严格 lint | `cargo clippy -p runquiry-ui -p runquiry-app --all-targets --locked -- -D warnings -A clippy::multiple-crate-versions -A clippy::print-stderr` | exit 0；仅仓库既有两项例外 | 命令输出 |
| 格式/空白 | `cargo fmt --check`；`git diff --check` | 均 exit 0、无输出 | 命令输出 |
| 文件规模 | 非空非注释生产行统计 | `render.rs=225`，`render_evidence.rs=92`，均小于 250 | 命令输出 |

注：沙箱内的首次 app 测试是 20/21；唯一失败 `resolves_post_start_loopback_port_holder`
在创建本地监听端口时被 `Operation not permitted` 阻断。同一锁定命令在真实本机回环环境
复验为 21/21，通过结果列于上表。

## 真实 WSLg/X11 验收

- 表面：`DISPLAY=:0 WAYLAND_DISPLAY=`；`xset q` 返回可用。
- 所有指针操作通过 `/home/linuxbrew/.linuxbrew/bin/xdotool` 的真实 X11 mouse down/move/up，
  不是程序化修改 `ResizableState`。
- 1280px 专属窗口初始 divider 约为 x=910（侧栏右缘约 x=223，主/详情约 687/370）；首次
  偏右命中 x=910 未拖动，随即在真实 handle 命中区 x=906 按下并拖到 x=740。拖后主/详情约
  517/540，两个面板仍可操作；深色中文切换后的重渲染仍保留 x=740。
- 专属 `setsid python3` 进程 PID 223585 真实持有三层长路径文件；PID 查询结果的 File locks
  证据在详情中连续换行、没有横向裁断，且主区未被挤压。截图中唯一本机用户名所在 source
  描述行已做确定性遮盖；其余文字未变。

| 场景 | 二进制可观察结果 | 截图（SHA-256） |
| --- | --- | --- |
| 1280px 初始 65/35 + 长路径 | divider x≈910；长 File locks 路径按行换行 | `resizable-final-before-drag-1280.png` `cefaf48d169ed0d04271061068a78d15f8dd91e41260542ed566d37af8fc317e` |
| 1280px 实际拖拽 | divider x≈740；主/详情均可读 | `resizable-final-after-drag-1280.png` `2b1a6092ac024bd15d074d07ba6f8a90638b3478da4f929daa769ce3556c925b` |
| 拖后深色中文重渲染 | 仍为 x≈740；中文与长路径显示正常 | `resizable-final-dark-zh-retained-drag-1280.png` `0d054d2a0815c66cc2fa298ffac9b8d7011d2061733c5ea93ed9323d85d56dbc` |
| 1100px 初始分栏 | divider x≈793；侧栏后约 570/307（65/35） | `resizable-final-1100-initial.png` `194332655c7eb346443db6bfa1feed80f2729d66e0896550c54ec938129e190a` |
| 1099px 紧凑边界 | 无 inline detail/divider，主区占满 | `resizable-final-1099-compact.png` `39e908fc1c050333e010d3f21aa61b9584bb20e52c17b886aa83a4863d2cfa14` |
| 960px Sheet | 选中真实 systemd/PID 1 后右侧 Sheet 出现 | `resizable-final-960-sheet.png` `6ccb81969e38fa8f3d30b6c106192fa87208b18c390e30b90a5688174fd74f66` |

截图目录：`docs/qa/batch4a/screenshots/`。

## 清理回执

- 已终止仅本轮启动的 Runquiry PID 219204、223295、225236 和临时文件持有 PID 223585；对应
  X11 window 搜索均无结果。
- 已删除精确专属目录 `/tmp/runquiry-resizable-1tjF3k`、
  `/tmp/runquiry-resizable-final-Qq2EWL`、`/tmp/runquiry-resizable-1100-7iY538`。
- 未触碰 `127.0.0.1:46245`；清理后它仍由 `codex app-serve` PID 1421 监听。
