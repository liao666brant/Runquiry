# Batch 4A 独立 Linux GPUI Smoke QA

日期：2026-09-04 18:18–18:22（Asia/Shanghai）

## 执行边界

- 当前构建：`target/debug/runquiry`；`stat` 显示构建时间 `2026-09-04 18:05:45 +0800`，没有晚于该构建的 `runquiry-app`/`runquiry-ui` 源文件。
- 启动命令：`env DISPLAY=:0 WAYLAND_DISPLAY= target/debug/runquiry`。
- 本次实例：PID `31137`，X11 window `0xa00001`（通过 `DISPLAY=:0 xdotool getwindowpid` 精确映射）。另有先前实例 PID `25862`，未操作、未停止。
- 真实窗口尺寸：1280×800、1099×700、960×640；截图均由 `DISPLAY=:0 import -window 0xa00001` 获取。持久化截图只保留脱敏后的 `smoke-*` 八张；进程表“用户”列及详情中的运行用户已遮盖为 `[REDACTED]`。
- 本 smoke 未启动容器、监听器或锁测试进程，也未执行 100k 行视觉宣称。

## 场景矩阵

| 场景 | 精确调用与观察 | Verdict | 证据 |
|---|---|---|---|
| S1 Processes 表 | 窗口聚焦后 `xdotool key --window 0xa00001 ctrl+1`；等待 3–4 s。1280×800 显示真实 systemd/init 等进程行、PID、用户、健康列，并显示真实权限 issue。 | PASS | `smoke-processes.png` |
| S2 Processes 详情 | 在 1280×800 对表格第一行执行 `xdotool mousemove --window 0xa00001 330 377 click 1`；右侧详情显示 PID 1、概览、资源采样中、祖先/启动来源、Socket、文件锁、环境变量及 issue。 | PASS | `smoke-process-detail.png` |
| S3 Ports 表 | `xdotool key --window 0xa00001 ctrl+2`；显示真实 LISTEN socket 的协议、地址、端口、状态、PID/进程和公开绑定，权限 issue 可见。 | PASS | `smoke-ports.png` |
| S4 Ports 详情 | 在第一批真实行上点击 `330,378`；详情显示 protocol/address/port/status/PID/process/public bind 和 issue。 | PASS | `smoke-ports-detail.png` |
| S5 Containers 状态 | `xdotool key --window 0xa00001 ctrl+3`；本机无可用 docker/其它运行时，界面显示“无法加载工作区”与具体受限/工具诊断，无虚构容器行。 | PASS（诚实 unavailable） | `smoke-containers.png` |
| S6 File Locks 表与详情 | 点击侧栏 File Locks（`xdotool mousemove --window 0xa00001 50 263 click 1`），表显示真实 lock/open-file 行、PID、进程、FD/类型/模式及部分失败 issue；点击 `330,378` 后详情显示路径、PID、进程、FD、类型、模式和 3 项 issue。 | PASS | `smoke-file-lock-detail.png` |
| S7 1099 Sheet | `xdotool windowsize 0xa00001 1099 700`，Processes 加载后点击 `300,380` 并按 Return；详情以右侧 Sheet 出现，未使用 65/35 内联详情。 | PASS | `smoke-1099-sheet.png` |
| S8 960 Sheet、Escape、稳定选择 | `xdotool windowsize 0xa00001 960 640`，切到 Ports，点击首行；Sheet 显示。按 Escape，再按 Down、Return；选中从 `127.0.0.53:53` 移至 `127.0.0.1:45678`，并重新打开对应详情。 | PASS | `smoke-960-focus-reopen.png` |
| S9 滚动稳定性 | 回到 1280×800 Processes，点击首行后执行 `xdotool key ... Page_Down`；表滚动条移动，选中行仍为可见且唯一的 `dbus-daemon` PID 136，右侧详情同步为同一 PID。 | PASS | `/tmp/runquiry-batch4a-smoke-1788517067/scroll-check.png`（临时，未持久化） |

## 证据与敏感信息检查

持久化截图使用 PNG 格式，尺寸分别为 1280×800（六张）、1099×700（一张）、960×640（一张）。人工检查后，进程表用户列和进程详情运行用户字段在涉及截图中已遮盖；截图未保留 token、环境变量值或敏感绝对路径。原始截图仅位于本次专属临时目录，完成后清理。

## 清理回执

- 仅向本次实例发送前台 `Ctrl-C`，终止 PID `31137`。
- `DISPLAY=:0 xdotool search --onlyvisible --name Runquiry getwindowpid %@` 不再返回 `31137`；PID `31137` 不再存在。
- 先前实例 PID `25862` 未触碰，按约束保留。
- `/tmp/runquiry-batch4a-smoke-1788517067/` 已清理；持久化脱敏截图保留在 `docs/qa/batch4a/screenshots/`。

## 结论

本次第一段 smoke 的真实窗口与键盘/鼠标交互均通过。Containers 的 unavailable 是当前 Linux 环境运行时缺失的真实能力状态，不是伪造数据。未发现需要直接修改产品代码的缺陷；未覆盖五类显式目标全量矩阵、100k 行、主题/语言切换和跨平台后端，这些留给完整 Batch 4A QA。
