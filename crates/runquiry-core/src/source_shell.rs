//! Shell / 用户工具来源判定与多路复用器富化（parity §5：witr `source/shell.go`）。

use crate::model::process::ProcessSummary;
use crate::model::source::{Source, SourceType};
use crate::port::source::SourceEvidence;

/// 交互 shell / 桌面启动器名单（parity：witr `isShell`；小写命令基名）。
const SHELLS: &[&str] = &[
    "bash",
    "zsh",
    "sh",
    "fish",
    "csh",
    "tcsh",
    "ksh",
    "dash",
    "ash",
    "cmd.exe",
    "powershell.exe",
    "pwsh.exe",
    "explorer.exe",
];

/// 用户工具名单（witr `userTools`：运行时、编辑器、终端）。
const USER_TOOLS: &[&str] = &[
    "python",
    "python3",
    "node",
    "ruby",
    "perl",
    "php",
    "go",
    "java",
    "cargo",
    "npm",
    "yarn",
    "make",
    "code",
    "cursor",
    "vim",
    "nvim",
    "emacs",
    "nano",
    "gnome-terminal-",
    "kitty",
    "alacritty",
    "wezterm",
    "konsole",
];

/// Windows 风格可执行扩展名（userTools 查询前先剥离）。
const EXE_EXTENSIONS: [&str; 4] = [".exe", ".cmd", ".bat", ".com"];

/// 命令基名（witr `filepath.Base`，仅按 `/` 切分）。
pub(crate) fn command_base(command: &str) -> &str {
    command
        .rfind('/')
        .map_or(command, |idx| &command[idx + 1..])
}

/// 判定是否交互 shell（入参为小写化的命令基名；Windows 报告的命令大小写
/// 不一致，匹配一律先小写化，parity `detectShell` 注释）。
pub(crate) fn is_shell(lower_base: &str) -> bool {
    SHELLS.contains(&lower_base)
}

/// userTools 名单查询（Windows 风格扩展名先剥离再查，parity `detectShell`）。
fn is_user_tool(lower_base: &str) -> bool {
    let mut lookup = lower_base;
    for ext in EXE_EXTENSIONS {
        if let Some(stripped) = lookup.strip_suffix(ext) {
            lookup = stripped;
            break;
        }
    }
    USER_TOOLS.contains(&lookup)
}

/// 在祖先链（排除目标自身）中从目标向根回溯，找最近的 shell 或用户工具。
///
/// 命中规则对齐 witr `detectShell`：shell 名单 → 剥扩展名的 userTools 名单
/// → python/node 前缀（带版本号或路径的解释器）。
pub(crate) fn detect_shell(
    ancestry: &[ProcessSummary],
    evidence: &SourceEvidence,
) -> Option<Source> {
    if ancestry.len() < 2 {
        return None; // 无祖先可回溯：目标自身不参与 shell 判定
    }
    for p in ancestry[..ancestry.len() - 1].iter().rev() {
        let base = command_base(&p.command);
        let lower_base = base.to_lowercase();
        let is_tool = SHELLS.contains(&lower_base.as_str())
            || is_user_tool(&lower_base)
            || lower_base.starts_with("python")
            || lower_base.starts_with("node");
        if is_tool {
            return Some(enrich_multiplexer(
                Source::new(SourceType::Shell).with_name(String::from(base)),
                ancestry,
                evidence,
            ));
        }
    }
    None
}

/// tmux / screen 多路复用器富化（parity `enrichMultiplexer`）：祖先链（排除
/// 目标自身）存在 tmux / screen 时把会话名写入描述。
fn enrich_multiplexer(
    source: Source,
    ancestry: &[ProcessSummary],
    evidence: &SourceEvidence,
) -> Source {
    for p in &ancestry[..ancestry.len().saturating_sub(1)] {
        let base = command_base(&p.command);
        if base == "tmux" || base.starts_with("tmux:") {
            // TMUX 值格式：/tmp/tmux-1000/<session>,<pid>,<idx>
            let description = find_env_var(ancestry, evidence, "TMUX").map_or_else(
                || String::from("tmux session"),
                |session| {
                    session.split(',').next().map_or_else(
                        || String::from("tmux session"),
                        |path| {
                            path.rfind('/').map_or_else(
                                || String::from("tmux session"),
                                |idx| format!("tmux session '{}'", &path[idx + 1..]),
                            )
                        },
                    )
                },
            );
            return source.with_description(description);
        }
        if base == "screen" || base.starts_with("SCREEN") {
            let description = find_env_var(ancestry, evidence, "STY").map_or_else(
                || String::from("screen session"),
                |session| format!("screen session '{session}'"),
            );
            return source.with_description(description);
        }
    }
    source
}

/// 环境变量回溯：从目标向祖先逐级查找，返回首个命中值（parity `findEnvVar`，
/// 目标优先）。
pub(crate) fn find_env_var<'a>(
    ancestry: &[ProcessSummary],
    evidence: &'a SourceEvidence,
    key: &str,
) -> Option<&'a str> {
    for p in ancestry.iter().rev() {
        let pairs = evidence
            .env_by_pid
            .iter()
            .find(|(pid, _)| *pid == p.identity.pid())
            .map(|(_, pairs)| pairs)?;
        for (k, v) in pairs {
            if k == key {
                return Some(v.as_str());
            }
        }
    }
    None
}

/// 读取指定进程的环境键值列表（证据缺失时为空切片）。
pub(crate) fn env_pairs<'a>(
    summary: &ProcessSummary,
    evidence: &'a SourceEvidence,
) -> &'a [(String, String)] {
    evidence
        .env_by_pid
        .iter()
        .find(|(pid, _)| *pid == summary.identity.pid())
        .map_or(&[], |(_, pairs)| pairs.as_slice())
}
