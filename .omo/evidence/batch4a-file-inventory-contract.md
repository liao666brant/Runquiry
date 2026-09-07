# Batch 4A P1：File Inventory 全量契约与 Linux 实现证据

## 交付结论

- `FileInventory` 新增 `list()`，`holders()` 改为返回 `FileInventoryEntry`。
- `FileInventoryEntry` 使用 `fd: Option<u32>` 与 `lock: Option<LockMetadata>`；
  `LockMetadata` 成对保存 `LockType`/`LockMode`，普通 FD 不再伪装成锁。
- Linux 全量清单包含所有成功 `readlink` 的 FD 目标与真实 `/proc/locks` 行；
  同一 `(pid, path)` 有真实锁时去掉普通 FD 行。
- `/proc/locks`、单进程 `comm`、FD 目录或单 FD 的局部失败返回 Partial 并保留其他数据；
  根 `/proc` 无法枚举时返回 `data = None`。
- 局部失败按 10 个稳定的阶段/错误类别聚合，每类仅返回计数与一个样本，诊断集合
  的严格上界为 10；十万条同类失败不会生成十万条 `DiagnosticIssue`。
- 锁路径解析的 FD 目录/readlink 失败不再静默；非法 PID 0 会被跳过并聚合为
  `ParseFailed`，不会伪装为 PID 1。
- `ProcessFileLocks::locks_of` 继续返回 `FileLockEntry`，锁语义未改变。

## Red / Green

| 场景 | 调用 | 二进制可观察结果 | 原始产物 |
|---|---|---|---|
| 绿色基线 | `cargo test -p runquiry-core --locked`；`cargo test -p runquiry-platform --locked` | 修改前 core 107、platform 90 个测试通过 | 本次执行会话输出（修改前） |
| 缺失公共契约 Red | `cargo test -p runquiry-core --locked --test file_inventory_contract` | exit 101；缺失 `FileInventoryEntry`、`LockMetadata` 与 `FileInventory::list` | `/tmp/runquiry-batch4a-p1-red-core.log` |
| 缺失 Linux 实现 Red | `cargo test -p runquiry-platform --locked --test linux_adapters file_inventory_lists_plain_fd_without_fabricating_lock` | exit 101；`FileInventory::list` 不存在 | `/tmp/runquiry-batch4a-p1-red-platform.log` |
| 审查修复 Red：有界诊断 | 定向运行十万条非法锁模式 + 失败 FD 测试 | exit 101；实际返回 `100001` 条 issue | 本文下方“审查修复” |
| 审查修复 Red：锁路径错误 | 定向运行 `locks_of` 缺失 FD 目录/坏链接测试 | exit 101；锁保留但 issues 为空 | 本文下方“审查修复” |
| 审查修复 Red：PID 0 | 定向运行非法 PID 测试 | exit 101；非法行被映射为有效条目 | 本文下方“审查修复” |
| Core Green | `cargo test -p runquiry-core --locked` | exit 0；107 个测试通过 | 本证据文件 |
| Platform Green | `cargo test -p runquiry-platform --locked` | exit 0；97 个测试通过 | 本证据文件 |
| 严格 lint | `cargo clippy -p runquiry-core -p runquiry-platform --all-targets --locked -- -D warnings` | exit 0 | 本证据文件 |
| 自有文件格式 | 对 14 个自有 Rust 文件执行 `rustfmt --edition 2024 --check` | exit 0 | 本证据文件 |
| Diff 健康 | `git diff --check` | exit 0，无输出 | 本证据文件 |

定向 Linux 契约测试覆盖：普通文件与 `pipe:[...]` 的 FD/无锁元数据、锁解析
Partial 保留普通行、一个 PID 的 FD 目录失败不丢其他 PID、holders 普通/真实锁、
真实锁优先去重，以及 `locks_of` 按 PID 保持原语义。

## 独立审查修复

- 有界诊断测试在修复前稳定失败：`十万条同类失败不得生成逐行诊断，实际
  100001 条`；修复后同一命令 exit 0，耗时约 0.21 秒，成功 FD 仍保留。
- `locks_of` 的缺失 FD 目录与不可 readlink 条目测试在修复前均因诊断缺失失败；
  修复后两项 exit 0，锁数据仍各保留 1 条且结果为 Partial。
- PID 0 测试在修复前因输出非空失败；修复后 exit 0，非法行被跳过并产生
  `ParseFailed`。
- 删除了自证式 `core/tests/file_inventory_contract.rs`；真实边界由 Linux 合成
  `/proc` 适配器测试与 `resolve_file_holders` 既有消费者测试共同覆盖。

## 真实 Linux QA

场景：审查修复后在 `/tmp/runquiry-p1-review-qa.*` 创建一个受控文件，启动一个只打开该文件的
短生命周期进程，并启动一个通过 `fcntl.lockf(LOCK_EX)` 持有 POSIX 写锁的受控
进程。随后调用生产 `LinuxPlatform::new()` 的可复用示例入口：

```text
cargo run -p runquiry-platform --locked --example linux_qa -- <受控文件>
```

二进制可观察结果：exit 0；全量清单观测到普通 FD 17 条、真实锁 1 条；目标过滤
观测到普通 FD 1 条、真实锁 1 条；`locks_of` 对受控锁进程观测到 1 条真实锁。
输出不含环境变量值，不打印全量文件清单路径。关键可观察输出：

```text
文件清单: 普通 FD 17 条, 真实锁 1 条, 诊断 1 条
受控目标 controlled-file: 普通 FD 1 条, 真实锁 1 条, 诊断 1 条
受控锁进程: locks_of 1 条, 诊断 0 条
cleanup=complete
```

清理回执：两个受控进程均已终止并 wait 回收，受控文件及唯一临时目录均已删除；
脚本二次检查输出 `cleanup=complete`。

## 集成门状态与边界

- `cargo fmt --check` 当前 exit 1，仅报告并行 B6 UI 文件未格式化；P1 自有文件的
  独立 rustfmt check 为绿色。原始日志：`/tmp/runquiry-batch4a-p1-fmt-check.log`。
- `cargo check -p runquiry-app --locked` 当前 exit 101，仅因并行 B6 UI 表格缺少
  `gpui::ParentElement` 导入；P1 自身的 `runquiry-platform` example 与 core/platform
  全部已编译。原始日志：`/tmp/runquiry-batch4a-p1-app-check.log`。应由 Batch 4A
  集成门在 B6 稳定后复跑，不在 P1 越权修改 UI。
- 首次使用 `flock(1)` 的受控 QA 在当前沙箱文件系统中未出现在 `/proc/locks`；
  改用内核 POSIX `fcntl` 锁后得到可观察真实锁。本实现仍按 `/proc/locks` 同时解析
  POSIX、FLOCK 与 OFDLCK，三类解析由合成适配器测试覆盖。

## 文件规模

生产/可复用 QA Rust 文件纯代码行均低于 250：core file port 47、core resolve 214、
Linux file_diagnostics 123、Linux locks 133、Linux open_files 133、linux_qa 160。

## 修复复审后的真实 Linux QA 补证

执行时间：2026-09-04。以下为本轮实际执行命令；临时目录由 `mktemp` 生成，
进程号仅保存在 `<PLAIN_PID>` / `<LOCK_PID>` 对应的 shell 变量中，未记录真实值。

```sh
QA_DIR="$(mktemp -d /tmp/runquiry-p1-evidence.XXXXXX)"
QA_FILE="$QA_DIR/controlled-file"
: > "$QA_FILE"

bash -c 'exec 9<> "$1"; exec sleep 30' runquiry-plain "$QA_FILE" &
PLAIN_PID=$!
python3 -c 'import fcntl,sys,time; f=open(sys.argv[1],"r+"); fcntl.lockf(f,fcntl.LOCK_EX); print("locked",flush=True); time.sleep(30)' "$QA_FILE" \
  > /tmp/runquiry-p1-evidence-lock-ready.log \
  2> /tmp/runquiry-p1-evidence-lock-error.log &
LOCK_PID=$!

for ATTEMPT in 1 2 3 4 5; do
  if rg -q '^locked$' /tmp/runquiry-p1-evidence-lock-ready.log; then break; fi
  sleep 0.2
done

cargo run -p runquiry-platform --locked --example linux_qa -- "$QA_FILE" \
  > /tmp/runquiry-p1-evidence-qa.log 2>&1
QA_EXIT=$?

kill "$PLAIN_PID" "$LOCK_PID" 2>/dev/null || true
wait "$PLAIN_PID" "$LOCK_PID" 2>/dev/null || true
unlink "$QA_FILE"
rmdir "$QA_DIR"

if test ! -e "$QA_FILE"; then FILE_ABSENT=yes; else FILE_ABSENT=no; fi
if test ! -e "$QA_DIR"; then DIR_ABSENT=yes; else DIR_ABSENT=no; fi
if kill -0 "$PLAIN_PID" 2>/dev/null; then PLAIN_ALIVE=yes; else PLAIN_ALIVE=no; fi
if kill -0 "$LOCK_PID" 2>/dev/null; then LOCK_ALIVE=yes; else LOCK_ALIVE=no; fi
```

命令退出与关键脱敏输出：

```text
qa_exit=0
文件清单: 普通 FD 17 条, 真实锁 1 条, 诊断 1 条
受控目标 controlled-file: 普通 FD 1 条, 真实锁 1 条, 诊断 1 条
受控锁进程: locks_of 1 条, 诊断 0 条
== QA 结束（普通用户权限；未打印任何环境变量值） ==
```

清理后的直接检查结果：

```text
file_absent=yes       # test ! -e "$QA_FILE"
dir_absent=yes        # test ! -e "$QA_DIR"
plain_pid_alive=no    # kill -0 "$PLAIN_PID" 返回非零
lock_pid_alive=no     # kill -0 "$LOCK_PID" 返回非零
```

本段未记录真实用户名、真实 PID、临时目录随机后缀、无关进程名称或全量路径。
