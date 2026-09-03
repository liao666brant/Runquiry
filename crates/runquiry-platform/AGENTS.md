# Runquiry Platform 平台层

[crates](../) / runquiry-platform

## 模块职责

平台采集与控制层：Linux/macOS/Windows 采集器、容器运行时集成、进程控制（terminate/kill/pause/resume/renice）与外部命令执行。当前为骨架（B2 起逐步实现，见 [模块 04 计划](../../.omo/plans/runquiry-gpui-desktop/04-linux-platform.md)）。

约束：只依赖 runquiry-core，为其端口（trait）提供平台实现；UI 不得绕过本层直接读取 /proc、调用 Win32 API 或运行 lsof/容器 CLI。

## 入口与启动

无独立入口。库 crate，`src/lib.rs` 仅有模块级文档注释（实现前为空）。

## 对外接口

未发现（骨架阶段）。规划为对 runquiry-core 平台端口 trait 的三平台实现；Windows 进程操作明确不支持（标记 Unsupported）。

## 关键依赖与配置

- 依赖：`runquiry-core`（workspace 继承）；无其他第三方依赖（骨架阶段）。
- 平台差异与采集路径语义见 [docs/witr-parity.md](../../docs/witr-parity.md) §10（平台差异）及各章节证据列。

## 测试与质量

- `cargo test -p runquiry-platform --locked`：10 个测试（tests/fake_backends.rs——基于 tests/support/ 的 Scenario 失败注入与 FakePlatform，覆盖正常/空/部分/权限/工具缺失/超时/格式损坏/PID 复用零动作）。真实采集实现属 B2/B3。
- lint 基线由根 `Cargo.toml` 的 `[workspace.lints]` 统一约束。

## 常见问题

- GPUI/Linux 构建的系统依赖（pkg-config、fontconfig、xkbcommon、wayland 等）在 A1 已安装；平台层新增系统库依赖时须同步记录。

## 相关文件清单

- `crates/runquiry-platform/Cargo.toml` — crate manifest
- `crates/runquiry-platform/src/lib.rs` — 库入口（骨架）
- `docs/witr-parity.md` — 采集与进程操作行为契约
- `.omo/plans/runquiry-gpui-desktop/04-linux-platform.md` — Linux 平台任务（B2）

## 变更记录

- 2026-09-02：初次索引。骨架状态，仅有 manifest 与 lib.rs 占位。
- 2026-09-03：Batch 2 A5——新增 tests/support/（Scenario 失败注入、FakePlatform 假实现）与 tests/fake_backends.rs；src/ 仍为骨架（真实实现属 B2/B3）。