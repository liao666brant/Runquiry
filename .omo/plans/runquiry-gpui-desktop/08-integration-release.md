# 模块 08：跨平台集成、硬化与分发

上级方案：[Runquiry 桌面化实施计划](../runquiry-gpui-desktop.md)

## 模块目标

在平台模块完成后统一验证行为契约，完成安全、性能、可访问性、打包和文档，并以真实安装产物作为 v1 完成依据。

## 所有权与边界

允许写入：

- 跨 crate 集成测试。
- 安全、性能和可访问性修复所涉及的现有模块。
- Packager.toml 或等价 cargo-packager 配置。
- CI 工作流。
- README、平台能力、安装、权限和许可证文档。
- 最终 QA 与发布证据。

共享文件修改必须由本模块唯一集成负责人串行处理，不允许多个平台 Agent 同时解决冲突。

不包含：

- Git push、远程 Release、签名、公证或应用商店发布。
- 自动更新、遥测或运行时网络功能。
- 未经行为契约批准的新产品功能。

## TODOs

- [ ] **C4 双平台契约回归**
  - 进度（2026-09-07，Batch 7B；**套件准备完成，验证阻断**）：三平台契约覆盖矩阵、差异复核与缺口清单落地（`.omo/evidence/batch7b-c4-matrix.md`）；确认提交前再门禁提取为 `ProcessActionFlow::confirm_if_usable` 并补 2 个门禁测试（闭合 H 类唯一本轮缺口）。环境盘点：无 macOS 主机；WSL 后发现 Windows 主机（rustup 1.98.0-msvc）待授权；交叉目标装在 1.98.0 而项目锁 1.95.0。C1/C2 编译与实机证据仍为零，C3/C4 新测试未运行——C4 保持未完成，v1 契约未冻结，不开始 D1/D2。
  - 2026-09-08：C1（macOS）已随 macOS 移出 v1 范围删除，本任务范围收窄为 Linux 与 Windows。
  - 进度（2026-09-10，Linux/WSL 侧）：在基线 `5a837d0` 上以锁定工具链 1.95.0 串行执行——`cargo fmt --check` 干净、core 109、platform 180（含 `windows_*` 纯解析 77）、ui 69、app 24 全部通过，`cargo test --workspace --all-targets --locked` 385/385 退出码 0，clippy 零 error，`check --examples` 通过（日志与元数据见 `.omo/evidence/c4-linux-verification.md`）。修复 ui 既有失败 `wide_layout_starts_with_a_65_35_split_after_the_sidebar`：`1e44f6e` 侧栏定为 120px 后测试仍按 `px(224.)` 计算，提取共享常量 `SIDEBAR_WIDTH` 统一实现与断言（三处硬编码收敛为一处）；并删除 `processes/tests.rs` 中 clippy `--all-targets` 暴露的未使用 `supported` 变量。**缺口闭合**：batch7b §6.1-2（纯解析套件已跑）、§6.1-3（ui C3 契约与门禁测试已跑）、§6.2 需 Linux 环境项。**C4 仍未完成**：Windows 侧须在 `5a837d0` 之后重跑（其全绿记录早于 macOS 移除提交，两平台须同基线）；差异 #1/#2 待裁决、#3/#4 待实机复核；Linux GUI 交互回归未做。
  - 依赖：C2、C3。
  - 在 Linux、Windows 运行同一 adapter contract suite。
  - 对比排序、过滤、五类目标、来源、告警、脱敏、刷新、设置和能力状态。
  - 差异必须归类为平台事实或缺陷；平台事实回写行为契约，缺陷回到对应模块修复。
  - 冻结 v1 行为契约后，后续任务不得改变语义。
  - 验证：cargo test --workspace --all-targets --locked 在两个系统均通过。
  - 完成证据：双平台对照表、差异处置、冻结契约版本。

- [ ] **D1 安全、性能与可访问性**
  - 依赖：C4。
  - 审核外部命令 argv、路径、输出上限、超时和子进程清理。
  - 审核 PEB/Win32 FFI 的 unsafe 边界。
  - 验证敏感环境变量、命令参数、错误信息和 QA 证据均无泄露。
  - 使用 100k 行 fixture、连续自动刷新、快速切换页面和选择验证 stale generation。
  - 验证 960×640、1280×800、超宽窗口、中英文、浅深主题、键盘焦点和对比度。
  - 主线程不得执行系统扫描；滚动和输入期间不得出现可感知冻结。
  - 完成证据：威胁检查表、性能记录、可访问性/视觉 QA、问题修复清单。

- [ ] **D2 跨平台打包流水线**
  - 依赖：C4。
  - 使用单一 cargo-packager 配置。
  - Linux x86_64 生成 AppImage 和 DEB。
  - Windows x86_64 生成 MSI。
  - 每个产物附 SHA-256、第三方许可证和构建元数据。
  - 干净 runner 只使用 Cargo.lock 和 --locked 构建。
  - 安装器保持无签名，不读取或要求签名密钥。
  - 验证：每个平台安装、启动、退出、卸载和离线重启。
  - 完成证据：产物清单、校验和、runner 日志、安装/卸载记录。

- [ ] **D3 用户与维护文档**
  - 依赖：D1、D2。
  - README 仅保留定位、能力摘要、支持平台和详细文档入口。
  - 独立文档说明平台能力、权限、容器 CLI、安装、无签名警告、故障排查和第三方许可证。
  - 记录查询实际版本和构建元数据的方法，不在长期文档手写当前值。
  - 保留 witr Apache-2.0 归属和 Runquiry GPL-3.0-or-later 说明。
  - 文档不得把通过编译表述为通过实机 QA。
  - 验证：内部链接、相对路径、标题锚点和许可证文件全部可解析。
  - 完成证据：文档索引、链接检查、许可清单。

## Final Verification Wave

- [ ] **D4 最终集成与发布门**
  - 依赖：D1-D3。
  - 由唯一集成负责人处理共享冲突并冻结 Cargo.lock。
  - 执行：
    - cargo fmt --check
    - cargo clippy --workspace --all-targets --all-features -- -D warnings
    - cargo test --workspace --all-targets --locked
    - cargo deny check
    - cargo build --workspace --release --locked
    - cargo tree -d
  - 验证 GPUI 和 GPUI Platform 只有一个 Git source。
  - 在 Linux X11、Linux Wayland、Windows 安装最终产物并完成四工作区、五类调查和平台能力场景。
  - 核对无 unwrap、expect、panic、未说明 unsafe 或占位 TODO。
  - 核对所有 QA 进程、容器、窗口、临时目录和安装测试资源均已清理。
  - 仅当行为契约、自动检查、实机 QA、安装包和许可证证据全部通过时标记完成。

## 全局失败条件

- 任一平台只有编译证据而无实机运行证据。
- 两套 GPUI source 同时存在。
- Unsupported 被显示为空数据或可执行按钮。
- 旧 generation 可以覆盖当前页面。
- 应用运行时发起未声明网络请求。
- 安装包缺少许可证、校验和或 witr 归属。
- QA 遗留进程、容器、窗口或临时资源。

## 最终交接格式

- 双平台自动验证命令和结果。
- 安装包、SHA-256 和许可证清单。
- 四工作区、五类调查、平台限制的实机证据。
- 安全、性能、可访问性结论。
- 清理回执。
- 已知限制；无则明确写“无”。
