# Stage 4 评审修复证据（2026-09-04）

基线提交：`984999ccf98527286002d6ae49e980c27988421c`。本文件覆盖其后的未提交修复；未执行 commit/push，未改根 manifest、Cargo.lock、deny.toml、rust-toolchain.toml 或 `witr/`。

## 修复范围

- CommandRunner：输出超限硬失败、运行中取消、读取错误传播、进程组终止与 wait 回收；sudo 下 Podman/nerdctl 恢复原普通用户。
- 容器：command/Compose 五字段匹配保留在私有非 serde 结构；nerdctl 稳定键为 containerd；Docker 发布端口回退；host PID 候选必须通过 Linux cgroup 归属验证。
- Linux：生产 PID 由 sysinfo 枚举，构造墙钟有效；详情字段失败保留部分结果并逐项诊断；systemd D-Bus 设置方法超时并限制为单一 in-flight worker。
- 结构：拆分超长 core/platform 测试与 command runner 辅助职责；本轮涉及的 core/platform Rust 源码、测试均不超过 250 纯代码行。
- 计划与 AI 索引：同步模块 02/03/04 计划、根与 core/platform AGENTS.md。

## 自动验证

- `cargo test --workspace --locked`：通过，共 232 个（core 107、platform 90、ui 21、app 14）。
- `cargo test -p runquiry-platform --test command_runner --locked`：10/10 通过。
- 双流竞争用例独立连续运行 10 次：10/10 通过，无间歇失败。
- `cargo clippy -p runquiry-core -p runquiry-platform --all-targets --locked -- -D warnings`：通过，零警告。
- `cargo clippy --workspace --all-targets --locked`：退出码 0；只报告锁定 GPUI 树的 77 条既有重复版本提示，以及 app 两处既有诊断 `eprintln!` 提示。
- `cargo check -p runquiry-app --locked`：通过。
- `cargo fmt --all -- --check`、`git diff --check`：通过。
- `cargo deny check`：退出码 0；advisories、bans、licenses、sources 均 ok。锁定 GPUI 树仍报告既有 duplicate/no-license-field 提示。
- core/platform 生产源码禁用宏扫描：未发现 `unwrap`、`expect`、`panic!`、`todo!`、`unimplemented!` 或 `unreachable!`。
- 严格全 workspace Clippy 会把 runquiry-ui 的既有 GPUI `multiple_crate_versions` 与 app 的既有诊断 `eprintln!` 提示提升为错误；本轮变更所属 core/platform 已用上述分包严格门禁覆盖。
- Rust LSP daemon 多次请求均在 30 秒超时；以成功的编译、全量测试及严格 core/platform Clippy 作为诊断替代证据。

## 真实 QA

- `cargo run -p runquiry-platform --example linux_qa --locked`：真实 `/proc` 返回 14 条进程、基线诊断 0，自身排除成功；详情、子进程、Socket、端口、文件锁路径均执行，环境变量只打印数量。详情携带 1 条字段诊断且数据保留。
- `cargo run -p runquiry-platform --example container_qa --locked`：本机检测到 docker CLI，但 daemon/list 返回退出码 1；其余六类 CLI 缺失。结果为 Partial + 逐运行时 `external_tool_failed`，无伪造容器数据。当前环境不能重验正向 Docker daemon 路径，较早正向证据保留在模块 03 历史记录中。
