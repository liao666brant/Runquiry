# 模块 01：工程基线、行为契约与测试资料

上级方案：[Runquiry 桌面化实施计划](../runquiry-gpui-desktop.md)

## 模块目标

建立其他模块共同依赖的 Workspace、依赖与许可基线，形成可追溯的 witr 行为契约，并提供不包含本机敏感数据的跨平台测试资料。

## 所有权与边界

本模块负责：

- 根 Cargo.toml、Cargo.lock、rust-toolchain.toml、deny.toml。
- 四个 crate 的 manifest 与最小目录骨架。
- LICENSE、NOTICE、第三方许可证生成配置。
- docs/witr-parity.md。
- tests/fixtures/ 及共享测试 fixture 定义。

本模块不负责：

- 业务采集、分析算法或产品页面实现。
- 修改 witr/ 参考源码。
- GPUI 产品界面和平台 API 细节。

共享根文件只允许 A1 负责人写入。其他任务需要新增依赖时提交明确需求，由 A1 负责人集中修改并重新验证单一 GPUI source。

## TODOs

- [x] **A1 依赖、许可与 Workspace 基线**
  - 依赖：无。
  - 建立 runquiry-core、runquiry-platform、runquiry-ui、runquiry-app 四层 Workspace。
  - 保留 Rust edition 2024，使用稳定工具链；从 Rust 1.90.0 起验证，锁定第一个可编译依赖基线的精确 patch 版本。
  - 将 gpui-component 固定到上级方案指定提交，并通过 Cargo.lock 将 Zed GPUI 固定到指定提交。
  - 初始化 gpui-component，并保证 Root 是窗口第一级视图。
  - 添加 GPL-3.0-or-later 项目许可证、witr Apache-2.0 NOTICE 和第三方许可证生成配置。
  - deny.toml 只允许固定来源的 zlog、ztracing、ztracing_macro 许可证例外。
  - 验证：
    - cargo check -p runquiry-app --locked
    - cargo tree -p runquiry-app -d
    - cargo deny check
    - 在 Linux 实际打开并关闭最小窗口
  - 完成证据：精确工具链版本、GPUI source 列表、许可证检查结果、窗口启动记录。
  - 实施记录（2026-09-02）：
    - 工具链锁定 1.95.0：1.90.0/1.91.0 低于 oo7@0.6.0 的 rust-version=1.92；1.92.0 缺 gpui_util 的 `slice_as_array`；1.93.0/1.94.0 缺 gpui 的 `cold_path`；1.95.0 起完整编译。
    - Zed GPUI 经 `cargo update -p gpui --precise` 锁定 f66ed399cdde86092af8af3dc7b418abf45f37f8，锁内 23 个 zed 包单一 source，无 Glass-HQ；gpui-component 91217366a5765600a127bf108ce00b7143a93381。
    - cargo-deny 0.20.2 全绿（advisories/bans/licenses/sources ok）；GPL 例外仅 zlog/ztracing/ztracing_macro/workspace-hack（固定 zed 提交）与 Runquiry 自身。
    - cargo-deny 限制：bans 无法解析 git 源的 workspace 继承依赖，故 gpui 等四个 git 依赖在 runquiry-app 内联声明（版本与根 workspace 说明一致）。
    - 窗口启动：WSLg X11 后端，窗口 "Runquiry" 1280×945 映射可见（截图确认），SIGTERM 正常关闭；libEGL 警告为 WSLg 软件渲染回退。
    - 工具链阶梯验证使用独立构建目录（`CARGO_TARGET_DIR=target-verify`，root 与 docs 下各有一份，均已 gitignore）。

- [x] **A2 witr 行为契约**
  - 依赖：无。
  - 逐项读取 witr 的 pkg/model、internal/target、internal/pipeline、internal/proc、internal/source、internal/tui 及对应测试。
  - 在 docs/witr-parity.md 记录四工作区、五类目标、来源优先级、告警、容器运行时、刷新策略、进程操作和平台差异。
  - 每项只使用 parity、intentional change、out of scope 三种状态。
  - 每个条目必须链接到具体源码文件、符号或测试，不链接仓库根目录。
  - 不复制整段 Go 实现，不将当前机器运行态写入文档。
  - 验证：检查每项产品范围和平台差异均有来源，文档内部链接全部可解析。
  - 完成证据：行为矩阵条目数、无来源条目列表必须为空、链接检查结果。
  - 实施记录（2026-09-02）：docs/witr-parity.md 共 202 条（§1 领域模型 35、§2 目标 40、§3 解析 20、§4 管线 27、§5 来源 21、§6 告警 12、§7 容器运行时 16、§8 刷新 6、§9 进程操作 9、§10 平台差异 5、§11 其他 7、§12 四工作区 4）；无来源条目 0；文档内相对链接已逐一在磁盘核实（183 个路径），符号名经抽查与 witr 源码一致。
  - 后续核定（2026-09-13）：契约最终为 205 条 / 12 节（§9 进程操作由 9 增至 12：新增 KillTree、行右键菜单与「打开文件位置」三条；其余章节分布不变），见 docs/witr-parity.md 末尾「验收核对」。

- [x] **A5 Fixture 与测试基础设施**
  - 依赖：A2、A3。
  - 从 witr 测试和人工构造数据建立 Linux、Windows fixture（原 macOS 一列已随 macOS 移出 v1 范围删除）。
  - 提供确定性时钟、固定 generation、假平台后端和失败注入入口。
  - fixture 覆盖正常、空、部分成功、权限失败、工具缺失、超时、格式损坏和 PID 复用。
  - 所有路径、用户名、环境变量和命令行均使用合成值。
  - 验证：
    - cargo test -p runquiry-core --locked
    - cargo test -p runquiry-platform --locked
    - 扫描 fixture，确认不包含开发机用户名、主目录、Token 或真实进程数据
  - 完成证据：fixture 清单、失败模式映射、敏感信息扫描结果。
  - 实施记录（2026-09-03，Batch 2）：
    - fixture：workspace 根 `tests/fixtures/{linux,macos,windows}/` 共 30 个 JSON（封套格式 platform/scenario/captured_at_epoch_ms/generation/capability/data/issues），清单与失败模式映射见 `tests/fixtures/README.md`。**2026-09-08 更新**：`macos/` 10 个 JSON 已随 macOS 移出 v1 范围删除，现存 `{linux,windows}/` 共 20 个。
    - 测试基建：`crates/runquiry-core/tests/support/`（fixture 装载器 + SocketEntry 边界校验 + 假平台后端）、`crates/runquiry-platform/tests/support/`（Scenario 枚举失败注入 + FakePlatform/命令执行假实现）、`crates/runquiry-core/tests/{fixtures_load,process_controller_contract}.rs` 与 `crates/runquiry-platform/tests/fake_backends.rs`。
    - ProcessController 契约修正（`crates/runquiry-core/src/port/process.rs` 文档注释）：execute 的身份参数表示确认流程持有的 expected snapshot；平台实现必须在动作前按 PID 重读 current identity 再用 same_process 比较；不新增第二个调用方身份参数；start_time 为 None（身份不可验证）时同样拒绝。FakeController 实现该语义：同 PID 不同 start_time → ProcessChanged 且动作计数 0；start_time None → 拒绝且计数 0；身份一致 → 执行（测试 process_controller_contract.rs 4 个 + fake_backends.rs 10 个）。
    - SocketEntry 输入边界（fixture DTO/loader 层，不改公共领域模型）：TCP/TCP6/UDP/UDP6 必须有合法端口（Some 且 1..=65535），Unix socket 必须无端口；后续平台真实输入必须复用该规则（已记录于 tests/fixtures/README.md）。
    - 敏感信息扫描：合成值约定（fxt- 前缀进程、fixture-user、/opt/runquiry-fixtures/…、容器 fxt…、固定 epoch 1700000000000）；grep 扫描 tests/fixtures/ 无开发机用户名/主目录/Token 命中（命令与结果见 tests/fixtures/README.md §3）。
    - 验证：cargo test -p runquiry-core --locked（36 个）/ cargo test -p runquiry-platform --locked（10 个）全过；clippy -D warnings 零警告。

## 模块退出条件

- A1、A2、A5 均已勾选。
- 其他 Agent 可以只读取上级方案、行为契约和 fixture 开始工作，无需重新决定依赖、许可证或 witr 语义。
- 工作区中只存在一套 GPUI/GPUI Platform Git source。

## 交接格式

- 变更文件。
- 固定的 Rust、gpui-component、Zed GPUI 版本。
- cargo check、cargo tree、cargo deny 和 fixture 测试结果。
- 尚未覆盖的 witr 行为；正常完成时应为“无”。
