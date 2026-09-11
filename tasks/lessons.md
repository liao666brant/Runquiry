# Lessons（本仓库特有教训）

- 2026-09-07：环境/验证相关的仓库文档（AGENTS.md、模块计划）只记录**平台级**事实（如「Windows 验证主机」「无 macOS 环境」），不得写入设备标识（主机名、型号、序列号）或可定位到具体机器的信息——用户明确要求 AGENTS.md 不记录设备相关内容。需要在会话中区分设备时，直接向用户确认，不落盘。
- 2026-09-07：给 FFI unsafe 块写安全注释时用 ASCII 冒号（`// SAFETY:`）。clippy 的 `undocumented_unsafe_blocks` 不识别全角「：」，24 处注释因此被误报为缺失。
- 2026-09-07：改用新工具链/新平台首次能编译后，应立即跑 `cargo clippy --all-targets`——被 cfg 门控的代码（Windows/macOS）此前从未过 clippy，本会话一次性暴露 51 处 deny 级问题（含内联 GetLastError、复合 unsafe 块、恒真断言等真实缺陷）。
- 2026-09-07：临时排查文件（重定向输出的 *.txt）用完即删，避免残留工作区（本会话曾累积 cc*.txt/t1.txt 等 9 个）。
- 2026-09-10：本机 shell 环境设置了 `RUSTUP_TOOLCHAIN=1.98.0`，它会**静默覆盖** `rust-toolchain.toml` 的 1.95.0 锁定——在仓库目录下 `rustc --version` 仍显示 1.98.0。运行任何验证前先确认工具链，本仓库统一用 `env -u RUSTUP_TOOLCHAIN cargo …` 执行，否则证据产生在错误版本上而不自知。
- 2026-09-10：WSLg 上做 GPUI GUI QA 必须 `env -u WAYLAND_DISPLAY DISPLAY=:0` 强制 X11——否则 WSLg 的 weston 接管 Wayland，窗口不出现在 X11 树里，`xdotool` 完全找不到；渲染需配 `LIBGL_ALWAYS_SOFTWARE=1`。另外 X11 下**首次点击会被窗口聚焦吞掉**，截图脚本要先发一次预热点击，否则首张截图仍是旧页面。
- 2026-09-10：不要用 `pkill -f '<仓库/二进制路径>'`——模式会匹配到执行该命令的 shell 自身（命令行含同样字符串），导致 shell 被杀、后续命令静默不执行。改用 `pgrep -a` 取 PID 后再 `kill <PID>`。
- 2026-09-10：`cargo test --workspace --all-targets --locked` 存在偶发 flaky——platform `command_runner::command_argv_elements_pass_through_verbatim_without_shell` 会以 `Text file busy (os error 26)` 失败（测试写出临时脚本后立即 exec，并发下命中 `ETXTBSY`；单跑该套件稳定 10/10）。判定 C4 全量是否通过时，遇到此用例失败应先重跑确认，不要径直判为回归。
- 2026-09-11：**用户在场的实机 GUI 验证不得使用键鼠注入自动化**（SendKeys/click 按当时焦点投递）——本会话在焦点竞争中把输入误送进用户其他窗口（文件管理器地址栏等），替用户打开了非预期窗口，用户明确指出。此后 Windows 侧 GUI 走查一律人工操作 + 按需截图；WSL→Windows 只保留「启动进程、读窗口矩形、截图、查事件日志」等无副作用通道。
- 2026-09-11：ETXTBSY flaky 根因已实证（WSL2 内核写后 execve 瞬态，愈合 ≤ ~5.7ms，生产 spawn ~75ms 重试窗口已兜底，证据 `.omo/evidence/etxtbsy-flaky-analysis.md`），遇到该用例失败先单跑复验再判定，不要径直判为回归。
