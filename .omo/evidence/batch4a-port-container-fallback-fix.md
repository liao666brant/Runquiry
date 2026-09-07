# Batch 4A 端口容器回退修复

日期：2026-09-04

## 契约与修复

`docs/witr-parity.md` 的端口目标契约要求：socket 已存在但属主不可知时，按发布端口命中的 Docker 容器必须作为容器 fallback 呈现；不能因为没有可验证宿主 PID 而再次返回 `SocketOwnerUnknown`。

`PlatformBackend::resolve` 的 `QueryTarget::Port` 分支现在把该错误交给
`resolve_published_port_container`。该方法复用 `container_target`：可验证且仍在当前进程清单中的 PID 仍映射为 `InvestigationTarget::Process`；缺少或无法验证的 PID 生成 `InvestigationTarget::Container`。`published_on` 的诊断作为 inherited issues 保留在 container detail。空发布查询仍返回原始 `SocketOwnerUnknown`，多个已发布容器仍保留 `Resolution::Ambiguous`。

## RED → GREEN

1. RED：新增 `backend::container::tests::published_port_without_verified_host_pid_resolves_to_container_fallback` 后执行：

   ```text
   cargo test -p runquiry-app --locked published_port_without_verified_host_pid_resolves_to_container_fallback -- --exact
   error[E0432]: unresolved import `super::published_container_resolution`
   ```

   失败原因是旧实现没有能把已发布容器（无可验证 PID）解析为调查目标的路径。

2. GREEN：实现该生产路径后执行：

   ```text
   cargo test -p runquiry-app --locked backend::container::tests::published_port_without_verified_host_pid_resolves_to_container_fallback -- --exact
   running 1 test
   test backend::container::tests::published_port_without_verified_host_pid_resolves_to_container_fallback ... ok
   ```

   可观察结果：唯一候选是 `InvestigationTarget::Container`，`verified_host_pid` 为 `None`，且保留 `ExternalToolFailed` 诊断；若退化为进程或歧义，测试返回错误。

## 验证

| 场景 | 调用 | 二进制可观察结果 |
|---|---|---|
| 应用全部单元/真实受控端口与文件目标 | `cargo test -p runquiry-app --locked`（非沙箱，回环端口在沙箱会被 EPERM 拒绝） | 21 passed, 0 failed |
| 四 crate 回归 | `cargo test -p runquiry-core -p runquiry-platform -p runquiry-ui -p runquiry-app --locked`（非沙箱） | core 107、platform 97、ui 47、app 21 全部通过 |
| 应用编译检查 | `cargo check -p runquiry-app --locked` | exit 0 |
| 格式与差异空白 | `cargo fmt --check`；`git diff --check` | exit 0 |
| 严格 Clippy | `cargo clippy -p runquiry-core -p runquiry-platform -p runquiry-ui -p runquiry-app --all-targets --locked -- -D warnings -A clippy::multiple_crate_versions -A clippy::print_stderr` | 受共享工作区中进行中的 UI 修改阻断：`crates/runquiry-ui/src/shell/render.rs:49,298` 的 `u32 as f32` precision lint；未改动本修复负责文件 |

## 范围与体量

- 仅改动本次负责的未跟踪 Batch 4A 文件：`crates/runquiry-app/src/backend.rs` 与 `crates/runquiry-app/src/backend/container.rs`。
- `backend.rs` 生产纯代码 220 行，`container.rs` 测试前生产纯代码 110 行，均低于 250 行约束。
- 未安装容器运行时、未联网、未提交或推送；未产生需清理的临时文件。
