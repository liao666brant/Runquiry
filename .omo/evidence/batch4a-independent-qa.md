# Runquiry Batch 4A 独立手工 QA

- 执行时间：2026-09-04
- 执行者：独立 QA executor
- 构建：`cargo build -p runquiry-app --locked`，退出码 0；随后使用当前 `target/debug/runquiry`
- 表面：真实 Linux GPUI 桌面窗口，`DISPLAY=:0 WAYLAND_DISPLAY=`，窗口实际尺寸 1280×800 与 960×640
- 约束：未修改 Rust 源码、依赖、计划或既有实现者证据；截图由本轮独立运行产生。Linux 运行环境中多个 `/proc/PID/exe` 访问被拒绝，应用按 partial issue 呈现。

## 结论

**REQUEST_CHANGES**。真实窗口和四个工作区均可启动并显示数据，但显式端口/文件目标调查没有解析到刚创建的受控对象，唯一名称目标被执行器包装命令污染为四个候选；这两个现象直接阻断 B5 的五类目标验收，不能判定 Batch 4A 通过。

## 场景记录

| 场景 | 步骤与观察 | 判定 | 证据 |
|---|---|---|---|
| S1 四工作区真实数据 | 启动当前二进制；`Ctrl+1..4` 依次进入 Processes、Ports、Containers、File Locks。Processes 显示 135/136 项及权限 partial issues；Ports 显示 8 个监听 socket，包含受控 `127.0.0.1:45678`/PID 13187；Containers 显示运行时不可用的能力态；File Locks 显示 5 条打开/锁记录及 partial issues。 | PASS（能力态诚实；不代表所有目标解析通过） | `independent-processes-1280.png`, `independent-ports-1280.png`, `independent-containers-1280.png`, `independent-file-locks-1280.png` |
| S2 Processes 详情 | 在 Processes 真实表格选择 systemd/PID 1；详情显示 Overview、Resources、Ancestry and children、Source、Warnings、Sockets、File locks、Environment variables，并显示权限问题。 | PASS | `independent-process-detail-1280.png` |
| S3 PID=0 无效目标 | 选择 PID 模式，经 `Ctrl+K` 输入 `0` 回车；右侧 Sheet 显示“调查失败 / invalid pid: must be a positive integer”。 | PASS | `independent-pid-zero-960.png` |
| S4 名称目标唯一性/多结果 | 创建两个 `runquiry-qa-ambiguous` 与一个 `runquiry-qa-unique` 受控 sleep；名称调查显示 `runquiry-qa-unique` 有 4 个候选（含执行器包装命令 PID 12249、12256、12257、12263），没有唯一结果。 | FAIL，高 | `investigate-unique.png`（raw 本轮临时证据；现象也见最终报告描述） |
| S5 端口目标 | 受控 `nc` 在 `127.0.0.1:45678` LISTEN，Ports 表可见 PID 13187；选择 Port 模式输入 `45678` 回车，详情显示“未找到匹配进程”。 | FAIL，高 | `independent-ports-1280.png`；复现步骤和观察见本报告 |
| S6 文件目标 | 受控 `/tmp/.../qa-lock.txt` 被打开并加 flock；选择 File 模式输入该路径回车，详情显示“未找到匹配进程”。 | FAIL，高 | `independent-file-locks-1280.png`；复现步骤和观察见本报告 |
| S7 B6 详情/选择 | Ports 表显示协议、地址、端口、状态、PID、进程、公开绑定；选择行后保留稳定高亮。File Locks 表显示 Path、Type、Mode、PID、Process、FD；Containers 在无可用运行时时显示限制与缺失工具列表。 | PASS_WITH_LIMITS | `independent-ports-1280.png`, `independent-file-locks-1280.png`, `independent-containers-1280.png` |
| S8 宽窄窗口 | 宽窗口 1280×800 显示列表/详情 65/35；调整到 960×640 后详情从主区移除并可使用 Sheet，未见内容越界。 | PASS | `independent-processes-1280.png`, `independent-processes-960.png` |
| S9 Sheet/Escape/焦点 | 960×640 下 PID=0 调查打开右侧详情 Sheet；按 Escape 关闭，回到工作区；输入框再次可聚焦。 | PASS | `independent-pid-zero-960.png`, `independent-processes-960.png` |
| S10 主题与语言 | 通过真实窗口相对坐标切换 Dark 与 中文；界面变为深色，工作区、表头、状态栏均显示中文，未见 CJK 裁切。 | PASS | `independent-dark-zh-960.png` |
| S11 快捷键 | 真实发送 `Ctrl+1`、`Ctrl+2`、`Ctrl+3`、`Ctrl+4`，四工作区均切换；`Ctrl+K` 聚焦调查入口；Escape 关闭 Sheet。 | PASS_WITH_LIMITS | 四工作区截图、`independent-pid-zero-960.png` |
| S12 表格滚动/稳定选择 | Ports 表真实点击行并发送 PageDown；数据区保持可见、选中行保持蓝色稳定高亮。当前数据量不足以证明 100k 行虚拟化。 | PASS_WITH_EXPLICIT_LIMITS | `ports-scrolled.png`（raw 本轮临时证据）、`independent-ports-1280.png` |

## manualQa

### surfaceEvidence

| scenario id | criterion reference | surface | exact invocation | verdict | artifactRefs |
|---|---|---|---|---|---|
| S1 | B5/B6 四工作区真实 Linux 数据/能力态 | GPUI desktop | `env DISPLAY=:0 WAYLAND_DISPLAY= target/debug/runquiry`; `Ctrl+1..4` | PASS_WITH_LIMITS | A1,A2,A3,A4 |
| S2 | B5 详情字段与 partial issues | GPUI desktop | 选择 Processes 首行 | PASS | A5 |
| S3 | 五类目标 invalid/zero | GPUI desktop | PID tab → `Ctrl+K` → `0` → Enter | PASS | A6 |
| S4 | 名称 unique/ambiguous | GPUI desktop | Name tab → `Ctrl+K` → `runquiry-qa-unique` → Enter | FAIL | A7 |
| S5 | 端口 unique | GPUI desktop | Port tab → `45678` → Enter；端口由 `nc -l 127.0.0.1 45678` 提供 | FAIL | A8,A2 |
| S6 | 文件 unique/lock | GPUI desktop | File tab → 受控 `qa-lock.txt` 路径 → Enter | FAIL | A9 |
| S7 | B6 字段、排序/筛选/选择 | GPUI desktop | `Ctrl+2/4/3`，点击表格行、PageDown | PASS_WITH_LIMITS | A2,A3,A4,A10 |
| S8 | 1280/960 断点 | GPUI desktop | `xdotool windowsize <RunquiryWindow> 1280 800/960 640` | PASS | A1,A11 |
| S9 | Sheet、Escape、焦点恢复 | GPUI desktop | 960×640 → PID=0 → Enter → Escape | PASS | A6,A11 |
| S10 | light/dark、en/zh-CN | GPUI desktop | 点击 Dark、中文控件 | PASS | A11 |
| S11 | Ctrl/Cmd+K、Ctrl/Cmd+1..4、Escape | GPUI desktop | `xdotool key ctrl+k`, `ctrl+1..4`, `Escape` | PASS_WITH_LIMITS | A1-A4,A6 |
| S12 | 真实滚动与稳定选择 | GPUI desktop | Ports 表点击行 → `Page_Down` | PASS_WITH_EXPLICIT_LIMITS | A10 |

### adversarialCases

| scenario id | criterion reference | adversarial class | expected behavior | verdict | artifactRefs |
|---|---|---|---|---|---|
| ADV1 | 目标解析稳定性 | invalid/zero PID | 拒绝非正 PID并显示结构化错误 | PASS | A6 |
| ADV2 | 目标解析稳定性 | ambiguous name | 列出完整候选，不自动选第一项 | FAIL | A7 |
| ADV3 | 目标解析稳定性 | unique port with live listener | 解析到端口属主或诚实显示端口专属错误 | FAIL | A8 |
| ADV4 | 目标解析稳定性 | unique locked file | 解析到持有者并保留锁元数据，或诚实显示文件专属错误 | FAIL | A9 |
| ADV5 | 容器能力边界 | no local container runtime | 显示 Unsupported/partial，不伪造容器数据 | PASS | A3 |
| ADV6 | 进程生命周期 | process disappears / stale selection | 保留稳定选择并显示 stale，不跳到其他行 | NOT_RUN | A12 |
| ADV7 | 敏感信息 | default redaction/reveal session | 默认脱敏，揭示仅当前详情会话 | NOT_RUN | A5 |
| ADV8 | 规模 | 100k rows | 只渲染可见范围并支持滚动 | NOT_RUN | A10 |
| ADV9 | CJK/响应式 | 960×640 中文深色 | 不裁切、不越界、Sheet 可关闭 | PASS | A11 |

未运行项不作为通过依据；因已有高严重级别目标解析失败，本轮直接 REQUEST_CHANGES。ADV6、ADV7、ADV8 需要修复后重新执行，其中 ADV8 当前真实 UI 没有可达 100k 注入 seam，不能伪称通过。

## artifactRefs

| id | kind | description | path |
|---|---|---|---|
| A1 | screenshot | Processes 1280×800，真实 Linux 进程表 | `docs/qa/batch4a/screenshots/independent-processes-1280.png` |
| A2 | screenshot | Ports 1280×800，含受控 45678 监听 | `docs/qa/batch4a/screenshots/independent-ports-1280.png` |
| A3 | screenshot | Containers 1280×800，诚实不可用能力态 | `docs/qa/batch4a/screenshots/independent-containers-1280.png` |
| A4 | screenshot | File Locks 1280×800，真实打开/锁列表 | `docs/qa/batch4a/screenshots/independent-file-locks-1280.png` |
| A5 | screenshot | Processes 详情字段与 partial issues | `docs/qa/batch4a/screenshots/independent-process-detail-1280.png` |
| A6 | screenshot | 960×640 深色中文 PID=0 失败 Sheet | `docs/qa/batch4a/screenshots/independent-pid-zero-960.png` |
| A7 | observation | 名称调查四候选异常；raw 截图按清理要求已删除 | `.omo/evidence/batch4a-independent-qa.md` |
| A8 | observation | Port=45678 未找到匹配进程；raw 截图按清理要求已删除 | `.omo/evidence/batch4a-independent-qa.md` |
| A9 | observation | File 锁路径未找到匹配进程；raw 截图按清理要求已删除 | `.omo/evidence/batch4a-independent-qa.md` |
| A10 | screenshot | Ports 表选择/滚动状态 | `docs/qa/batch4a/screenshots/independent-ports-1280.png` |
| A11 | screenshot | 960×640 深色中文响应式界面 | `docs/qa/batch4a/screenshots/independent-dark-zh-960.png` |
| A12 | observation | 本轮未执行进程退出 stale 场景，不能作为 PASS | `docs/qa/batch4a-independent-qa.md` |

## 缺陷

1. **高：Port 目标解析失败。** 复现：启动 `nc -l 127.0.0.1 45678`，Ports 表显示 45678/PID 13187；在 Processes 调查栏切换 Port，输入 `45678` 并回车；右侧显示“未找到匹配进程”。期望是解析到 PID 13187 或返回端口目标专属、可解释的 owner-unavailable 错误，而不是泛化“未找到匹配进程”。
2. **高：File 目标解析失败。** 复现：打开并 flock `/tmp/runquiry-batch4a-independent-Ib1v6q/qa-lock.txt`，File Locks 表可见打开/锁数据；切换 File，输入该绝对路径并回车；右侧显示“未找到匹配进程”。期望是返回持有者/锁元数据或文件能力错误。
3. **中：受控名称目标被执行器包装命令污染。** `runquiry-qa-unique` 返回四个候选，其中包含 QA 启动包装器 PID，无法证明唯一名称行为；需要在真实执行器命令行上隔离包装命令匹配或在目标采集层排除包装器。

## 清理回执

- 本轮记录并仅管理的 Runquiry PID：13340；本轮启动的辅助 PID：受控 `runquiry-qa-unique`/`runquiry-qa-ambiguous` sleep 与 shell 会话、回环 `nc` 45678。
- 清理动作：退出 Runquiry 窗口；结束本轮记录的辅助终端会话与回环监听；移除 `/tmp/runquiry-batch4a-independent-Ib1v6q/qa-lock.txt`、`qa-open.txt` 与临时目录；删除未复制的 raw 截图。
- 终态要求：`pgrep -a -f 'target/debug/runquiry$'` 无输出；`ss -ltn` 不再有 `127.0.0.1:45678`；临时目录不存在；无残余 Runquiry 窗口。
- 证据截图已最小化复制 8 张至 `docs/qa/batch4a/screenshots/`；该目录中的 PNG 均经 `file` 校验为真实 PNG，尺寸为 1280×800 或 960×640，未发现用户名、token 或环境变量值。

### 中断后资源清理补充（2026-09-04）

- 按父代理确认，仅向本轮回环监听器 PID `25703` 发送 `TERM`；验证 `ss -ltn '( sport = :45678 )'` 无监听。
- 删除专属目录 `/tmp/runquiry-batch4a-independent-repro-U4RGrO`，再次验证目录不存在。
- 明确核验 `127.0.0.1:46245` 仍由 codex app-server PID `1421` 监听，未触碰该进程或端口。
