# Lessons（本仓库特有教训）

- 2026-09-07：环境/验证相关的仓库文档（AGENTS.md、模块计划）只记录**平台级**事实（如「Windows 验证主机」「无 macOS 环境」），不得写入设备标识（主机名、型号、序列号）或可定位到具体机器的信息——用户明确要求 AGENTS.md 不记录设备相关内容。需要在会话中区分设备时，直接向用户确认，不落盘。
- 2026-09-07：给 FFI unsafe 块写安全注释时用 ASCII 冒号（`// SAFETY:`）。clippy 的 `undocumented_unsafe_blocks` 不识别全角「：」，24 处注释因此被误报为缺失。
- 2026-09-07：改用新工具链/新平台首次能编译后，应立即跑 `cargo clippy --all-targets`——被 cfg 门控的代码（Windows/macOS）此前从未过 clippy，本会话一次性暴露 51 处 deny 级问题（含内联 GetLastError、复合 unsafe 块、恒真断言等真实缺陷）。
- 2026-09-07：临时排查文件（重定向输出的 *.txt）用完即删，避免残留工作区（本会话曾累积 cc*.txt/t1.txt 等 9 个）。