# 模块 02：核心领域与调查分析

上级方案：[Runquiry 桌面化实施计划](../runquiry-gpui-desktop.md)

## 模块目标

实现与 UI、操作系统无关的领域模型、目标解析和调查分析管线，使所有平台共享同一套匹配、来源、告警、部分结果和错误语义。

## 所有权与边界

允许写入：

- crates/runquiry-core/src/
- crates/runquiry-core/tests/

不得写入：

- runquiry-platform、runquiry-ui、runquiry-app。
- 根 Cargo 配置和 Cargo.lock。
- 任何直接系统调用或 GPUI 类型。

需要新依赖时向 A1 负责人提出需求，不直接修改共享 manifest。

## TODOs

- [ ] **A3 核心领域接口**
  - 依赖：A1、A2。
  - 先以编译失败测试锁定上级方案定义的 Pid、Port、ContainerKey、QueryTarget、ProcessIdentity、Inspection、CapabilityStatus、InspectError 和 ProcessAction。
  - 定义 ProcessInventory、ProcessDetailsProvider、NetworkInventory、ContainerInventory、FileInventory、ProcessController、CommandRunner。
  - traits 保持同步和职责单一；平台调用由上层后台执行，不在 core 引入 async runtime。
  - 公共结果使用结构化字段和稳定错误码，不以自由字符串承担控制流。
  - 核心模型可序列化为测试 fixture，但不定义持久化数据库格式。
  - 验证：
    - cargo test -p runquiry-core --locked
    - cargo clippy -p runquiry-core --all-targets -- -D warnings
  - 完成证据：公共 API 清单、依赖树中不存在 GPUI/OS crate、测试与 Clippy 结果。

- [ ] **B1 目标解析与分析管线**
  - 依赖：A3、A5。
  - Name：大小写不敏感模糊匹配；exact 时匹配进程名、完整命令参数或路径段的完整 token。
  - PID：只接受正整数。
  - Port：只接受 1-65535。
  - File：保留输入路径语义，由 FileInventory 解析使用者。
  - Container：按名称、镜像、命令、Compose project/service 匹配；exact 时要求字段完全相等。
  - 多结果返回 Ambiguous 和完整候选，不自动选择第一项。
  - 组合进程、祖先链、来源、容器、资源、Socket、文件和告警，允许子采集器部分失败。
  - 按上级方案维护来源优先级和告警纯函数。
  - 使用 ProcessIdentity 区分“进程退出”和“PID 被复用”。
  - 验证：
    - cargo test -p runquiry-core target --locked
    - cargo test -p runquiry-core pipeline --locked
    - cargo test -p runquiry-core warnings --locked
  - 完成证据：五类目标矩阵、来源/告警 fixture 对照、部分成功和 PID 复用测试结果。

## 模块退出条件

- core 不依赖 GPUI、平台 crate 或外部命令。
- 五类目标和所有告警均由 fixture 锁定。
- 平台模块只需实现 traits，不需要重复业务判断。

## 交接格式

- 公共类型与 trait 列表。
- 对平台实现者必须满足的前置/后置条件。
- 对 UI 暴露的状态与错误码。
- 自动验证命令和结果。
