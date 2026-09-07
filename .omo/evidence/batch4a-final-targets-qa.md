# Runquiry Batch 4A 最终显式目标真实 QA

- 日期：2026-09-04
- 执行方式：只读运行当前工作区构建产物；未修改 Rust、依赖、计划或平台采集实现
- 基线：`HEAD e0bf6617d0541e8aae0df644699982c0f073bb02` 加当前未提交工作区
- 构建：`cargo build -p runquiry-app --locked`，exit 0
- 二进制 SHA-256：`99794f9e720ffb56c1b25d41b8d85769d6421d6c7763b0c8c3b5b9185ec83e6b`
- 表面：真实 Linux GPUI，`DISPLAY=:0 WAYLAND_DISPLAY=`，本轮应用 PID `145512`、X11 window `18874369`、1280x800

## 结论

**显式目标语义 PASS**。当前构建在真实 GPUI 窗口中正确完成 Name、PID、Port 与 File 四种显式目标调查；两个同名进程返回完整候选而没有自动选择。此前独立 QA 记录的 Port/File 找不到属主问题，在本轮 fresh backend 构建中不再复现。本结论不替代 Batch 4A 的独立视觉门禁。

## 受控前提

所有标识均由本轮临时目录后缀 `ZCFYX9` 派生，不使用仓库中的固定名称：

| 用途 | 内核/系统证据 |
|---|---|
| 唯一 Name 与 PID | `/proc` Name `rqZCFYX9u`，PID `150245` |
| File holder | `/proc` Name `rqZCFYX9f`，PID `150566`；`lsof` 显示 FD 4 持有 `/tmp/runquiry-targets-ZCFYX9/held-ZCFYX9.log` |
| Name ambiguity | 两个独立进程的 `/proc` Name 均为 `rqZCFYX9a`，PID `150887`、`150980` |
| Port owner | `ss -H -ltnp "sport = :37247"` 显示 `127.0.0.1:37247` LISTEN，`nc` PID `151260` |

本环境 `/bin/sleep` 是按可执行文件名选择 applet 的 uutils 多调用二进制，复制为随机 basename 后会以 `coreutils: unknown program` 立即退出。为维持“唯一随机 `/proc` Name、无常驻包装器污染”的测试语义，本轮改用直接 `exec python3` 的单进程 helper，并通过 `prctl(PR_SET_NAME)` 设置随机 Name；文件 helper 也由同一进程直接持有文件。失败的两次 sleep 尝试均在启动时立即退出，无残留 PID。

## 场景与结果

| 场景 | 真实操作与可观察结果 | 判定 | 证据 |
|---|---|---|---|
| Name unique | Name 模式输入 `rqZCFYX9u` 并回车；详情标题显示 `rqZCFYX9u · PID 150245`，没有候选选择步骤。 | PASS | A1；执行时 raw 观察 |
| PID unique | PID 模式输入 `150245` 并回车；再次解析为 `rqZCFYX9u · PID 150245`。 | PASS | A1 |
| Port unique | Port 模式输入 `37247` 并回车；详情标题显示 `nc · PID 151260`，Socket 显示 `TCP 127.0.0.1:37247 · LISTEN`。 | PASS | A2 |
| File unique | File 模式输入受控绝对路径并回车；详情标题显示 `rqZCFYX9f · PID 150566`，与 `lsof` 的唯一持有 PID 一致。普通 open 文件没有伪造锁元数据。 | PASS | A3 |
| Name ambiguous | Name 模式输入 `rqZCFYX9a` 并回车；右侧显示“找到 2 个候选，请明确选择”，候选恰为 PID `150887` 和 `150980`；未打开任一候选详情。 | PASS | A4 |

## 旁路视觉观察

A1-A3 同时暴露一个不属于本子任务修复范围的宽窗布局问题：1280x800 下打开唯一显式目标详情后，调查标题与目标类型标签发生重叠，主表/告警列被压缩至近似单字符宽，而最右侧预留详情区为空。目标解析结果仍可从详情标题和字段读出，因此上表的语义判定保持 PASS；但该现象应由 Batch 4A 的独立视觉门禁作为产品缺陷单独裁决。本轮没有修改 Rust 代码。

## 截图证据

截图均来自上述当前构建与本轮窗口，文件签名为真实 8-bit PNG，尺寸均为 1280x800。为避免暴露本机用户信息，A1-A3 对“启动来源”中的本地会话/用户名区域做了确定性遮罩；调查标题、PID、Socket、告警和目标结果区域未修改。界面只显示环境变量数量，没有显示环境变量值。

| ID | 路径 | SHA-256 |
|---|---|---|
| A1 | `docs/qa/batch4a/screenshots/targets-final-name-pid.png` | `2e82e0d20c0c74f91b6eddd9ee53b0957b89bc48611208c43963757b6e1a014d` |
| A2 | `docs/qa/batch4a/screenshots/targets-final-port.png` | `e2a0b2c2084706a62f95294dfb42e50cebb249baf13673f967b1b571909b6212` |
| A3 | `docs/qa/batch4a/screenshots/targets-final-file.png` | `6e41cf2fc1f241771d99fd8b2f2b8b6bae99ff17779df6aab2e76dc5af13a7` |
| A4 | `docs/qa/batch4a/screenshots/targets-final-name-ambiguous.png` | `7baa86365d04b32e699c13cd5e5b6d98689885147ad79f751ef490b63f91f4c1` |

## 清理回执

- 通过各自的执行会话向本轮 PID `145512`、`150245`、`150566`、`150887`、`150980`、`151260` 发送中断；随后 `ps -p ...` 无输出。
- `xdotool search --pid 145512` 无输出，本轮窗口 `18874369` 已关闭；没有操作其他并行 Runquiry 窗口。
- `ss -H -ltnp "sport = :37247"` 无输出，随机 QA 端口已释放。
- 删除专属目录 `/tmp/runquiry-targets-ZCFYX9` 以及未入库的 `/tmp/runquiry-qa-target-*.png`；最终 `find` 无匹配。
- `127.0.0.1:46245` 仍由 `codex app-serve` PID `1421` 监听，未触碰该端口或进程。
- 仅保留本报告及上述 4 张脱敏截图。
