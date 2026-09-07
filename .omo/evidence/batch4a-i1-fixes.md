# Batch 4A I1 复审修复证据

日期：2026-09-04

## 结论

I1 五项阻断均已关闭。动态 Sheet、容器 fallback、三清单稳定键详情、AppSession generation 接线及生产快捷键契约均有源代码、自动化回归和当前构建真实窗口证据；未修改依赖、根配置或 `witr/`。

## 阻断逐项证据

1. 动态 Sheet：`CompactDetail` 持有并观察 `Entity<AppShell>`，宽/窄复用同一 `detail_content`。960px 同一 PID 1 的 Sheet 从 PID 占位更新为 systemd Analysis：`/tmp/runquiry-batch4a-i1-process-sheet-before.png`、`/tmp/runquiry-batch4a-i1-process-sheet-after.png`。
2. 容器 fallback：`InvestigationTarget::Container` 保留 summary、verified PID 与 issues；`backend::container::tests::unique_container_without_verified_host_pid_remains_a_container_result` 通过。当前机器无可列出容器，真实状态见 `/tmp/runquiry-batch4a-i1-containers-real-960.png`。
3. 三清单详情：稳定键从当前 snapshot 定位，缺失返回 stale；字段与 issues 的测试 3 项均通过。真实 Ports 与 File Locks 宽/窄证据见 `/tmp/runquiry-batch4a-i1-ports-detail-wide.png`、`/tmp/runquiry-batch4a-i1-ports-sheet-1099.png`、`/tmp/runquiry-batch4a-i1-files-detail-wide.png`、`/tmp/runquiry-batch4a-i1-files-sheet-960.png`。
4. generation：filter/mode/selection/sort 先中止当前 refresh gate，再推进 `AppSession`；三种 DataTable 的真实 sort event 回传 shell。假后端测试覆盖四类变化对旧 snapshot 的拒绝。
5. ProcessCommand：app 的 `cx.bind_keys` 直接消费 `ProcessCommand::bindings()`；`Ctrl+3` 定向发送到真实 X11 window 后进入 Containers，见 `/tmp/runquiry-batch4a-i1-shortcut-ctrl3.png`。

## 门禁与清理

- core 107、platform 97、UI 47、app 15：全部通过；日志 `/tmp/runquiry-batch4a-i1-{core,platform,ui,app}.log`。
- app check、strict clippy、fmt、diff：全部 exit 0；日志 `/tmp/runquiry-batch4a-i1-{check,clippy,fmt,diff}.log`。
- 生产 Rust 最大 pure LOC 232，均小于 250：`/tmp/runquiry-batch4a-i1-loc.log`。
- PNG 均为完整 RGB PNG，尺寸覆盖 960×700、1099×700、1280×800。
- 最终 PTY session 61973 已 Ctrl-C；`pgrep -a -x runquiry` 无输出。
