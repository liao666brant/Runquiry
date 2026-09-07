# Batch 4A 最终 Linux/WSLg 运行 QA

## 最终门禁 PASS

- 最终二进制 SHA-256：`b085bfa101fc617ace1f1126487af9458b6d2268efb636320a22e400fbdaedc3`
- 第三次连续会话：Runquiry PID `251939`，window `6291457`；`getwindowgeometry --shell`：`X=656 Y=1147 WIDTH=1280 HEIGHT=800`
- 启动：`setsid env DISPLAY=:0 WAYLAND_DISPLAY= XDG_CONFIG_HOME=/tmp/runquiry-final-proof3-C8F9D6/config target/debug/runquiry`；进程正常驻留，完成 QA 后由测试清理终止（非自然 exit 0）。

### surfaceEvidence

| 场景 | 精确操作与可观察结果 | 判定 | 截图 |
|---|---|---|---|
| 1280 选中+filter | 点击 Processes 首行，filter 输入 `systemd`；选中行、详情、非空 filter 同时可见 | PASS | `final-qa-proof3-selected-filter.png` |
| Divider | `xdotool mousemove --window 6291457 906 400 mousedown 1 mousemove --window 6291457 700 400 mouseup 1`；分界由约 909 移至约 584 | PASS | `final-qa-proof3-dragged-filter.png` |
| 拖后暗色中文 | 保持选中行、详情、filter=`systemd`，点击 Dark、中文；状态均保持，divider 保持拖后位置 | PASS | `final-qa-proof3-dark-zh-state.png` |
| 拖后英文 | 点击 English；Processes、选中 PID 1、详情加载态、非空 filter 与拖后 divider 保持 | PASS | `final-qa-proof3-light-en-state.png` |
| 双空 placeholder | 清空 query/filter，分别在中文、英文截图；两者均正确本地化 | PASS | `final-qa-proof3-zh-empty.png`, `final-qa-proof3-en-empty.png` |
| 960 Sheet/Escape | 最终补充会话在 960x640 真实点击 `systemd` 行，右侧 `Details` Sheet 可见；Escape 后 Sheet 消失 | PASS | `final-qa-proof4-960-sheet-open.png`, `final-qa-proof4-960-sheet-after-escape.png` |
| 1099 compact | `xdotool windowsize 6291457 1099 700`，无内联详情 | PASS | `final-qa-proof3-1099-compact.png` |

### 同 SHA 第一会话 smoke 复用

以下三张截图来自同一 SHA `b085bfa...` 的第一会话，不是 proof3 重跑：PID `233099`（Name helper `rqQA7K2u`）、Port `37427`（nc PID `233103`）、File PID `233101`（`/tmp/runquiry-final-qa-Fm5F5z/held-QA7K2.log`）。截图 SHA：PID `4b9e514bd4f7502e2784a24274d9a4fecb63756886855472ad89db1344e0e40a`；Port `26b220ac024ca556807ed5113c121f4441664506176b999f2f9dee2617c814b0`；File `1bca55189644843ae250be90efa867000d637b77962f55c42e2369ce68f85fe2`。第一会话清理已终止 Runquiry PID `239758`、helper PID `233099/233101/233103`，删除 `/tmp/runquiry-locale-final-qtdXkj` 与受控文件，端口 `37427` 已释放。

### adversarialCases

### File smoke 纠正（当前 SHA）

当前 SHA 的完整 File 证据以 proof5 为准，取代本报告前述第一会话 File loading 截图及其描述：Runquiry PID `258792`、helper PID `258791`（`rqFILE5`，持有 POSIX `flock(LOCK_EX)`），文件 `/tmp/runquiry-file-final-target.lock`。等待 6 秒后详情明确显示 `rqFILE5 · PID 258791`、目标路径和 `Flock/Write`。截图 `final-qa-proof5-file-complete.png` SHA `06241ac9e5915bd01122afe2b3559cc16f34182411c2c1fd279badf6fc379a2f`。已终止上述 PID；现场确认目录仅含 `config/runquiry/settings.json` 与 `app.log`，逐个 `unlink` 后从内到外 `rmdir`，最终目录与受控文件均不存在。

| 场景 | 预期 | 判定 | 证据 |
|---|---|---|---|
| stale localization | 中英文切换更新两个 placeholder | PASS | proof3 zh/en empty |
| state preservation | 非空 filter、选择、详情、workspace、divider 不丢 | PASS | proof3 dark-zh/light-en |
| divider hit test | 正确窗口相对坐标拖动改变分界 | PASS | proof3 dragged-filter |
| unavailable containers | 不安装引擎，显示真实 unavailable/error | PASS（早期同工作区实景；最终 SHA 未重复） | `batch4a-i1-containers-real-960.png` |
| 100k visual boundary | 真实 100k 行窗口滚动 | NOT RUN / residual；仅有 100k `Arc`/索引逻辑测试，不声称视觉通过 | `batch4a-b5-processes.md`、`batch4a-b6-workspaces.md` |

### 截图 SHA-256

```text
77e51c51542a63457e6070be72dbb490bc8e7fa272f7021cedbb2f599ffb4643  final-qa-proof3-1099-compact.png
5c492a842956ee5aa76427b107c7f85b7a501a6b844a636d30b4843a808dc08a  final-qa-proof3-dark-zh-state.png
14e40c1a98ed426139c862925ccfd87b2a93ba56c4572d275ab5c633961e172c  final-qa-proof3-dragged-filter.png
97496de7f7bf2c5a61e85d0d508558870770d06606203f8df6e91c40b4cd77d6  final-qa-proof3-en-empty.png
ef200a841d56365e83ab5530112efc3bd3fa702ff6b3def3b7a255f8b43a3653  final-qa-proof3-light-en-state.png
885ea3000cc675c445f544185f70cd3ed52fdf3d5872c63209fb38c0386795b5  final-qa-proof3-selected-filter.png
c66fa9b74e72770ca2c59fae9d45fe150ce51af06e30f3cdc5f9ed46eb0fd02a  final-qa-proof3-zh-empty.png
9b4b9f273d9cb132b7bd1f9167a753c27d075e13073eae3188df2197cf47180f  final-qa-proof4-960-sheet-open.png
d62ae12ff03cdd6a92abded3807e4d121ab920dc90a8be2eb55d0b8dc25e179c  final-qa-proof4-960-sheet-after-escape.png
```

### 960 Sheet 最终补充会话

- 最终 SHA 仍为 `b085bfa101fc617ace1f1126487af9458b6d2268efb636320a22e400fbdaedc3`。
- Runquiry PID `254372`，window `6291457`；初始 geometry `X=656 Y=1147 WIDTH=1280 HEIGHT=800`，随后 `xdotool windowsize 6291457 960 640`。
- 真实操作：`xdotool mousemove --window 6291457 350 375 click 1`；等待后观察到右侧 `Details`、关闭按钮、`systemd · PID 1` 详情；`xdotool key Escape` 后 Sheet 消失。
- `final-qa-proof4-960-sheet-open.png` SHA `9b4b9f273d9cb132b7bd1f9167a753c27d075e13073eae3188df2197cf47180f`；`final-qa-proof4-960-sheet-after-escape.png` SHA `d62ae12ff03cdd6a92abded3807e4d121ab920dc90a8be2eb55d0b8dc25e179c`。

### 清理回执

终止第三次会话 PID `251939`，删除 `/tmp/runquiry-final-proof3-C8F9D6`；终止 proof4 会话 PID `254372`，删除 `/tmp/runquiry-sheet-final-D15sDd`。两目录均已现场核查不存在；本轮无残留随机端口/文件。未触碰 PID `1421` 或 `127.0.0.1:46245`。

## 已取代尝试（不计入结论）

- 第一次：使用绝对坐标 Y=1500，超出 800px 窗口，拖拽无效。
- 第二次：相对坐标拖拽截图缺少选中行和非空 filter，证据不足。
- 第二次的 `final-qa-proof3-960-sheet.png` 实际未显示 Sheet，已由 proof4 的真实打开/关闭截图取代，不计入结论。
- 上述尝试及旧 FAIL 仅保留作审计背景，不参与最终门禁。
