//! macOS `launchctl` / `ps -E` 输出解析（纯逻辑：只依赖 std 与 runquiry-core）。
//!
//! 语义对齐 witr：`target/name_darwin.go`（label 校验、`launchctl print`
//! PID 解析）、`launchd/plist.go::GetServiceLabel`（blame 域/label 解析、
//! `launchctl list` 回退）、`extended_darwin.go::parseLaunchctlLimitLine`
//! （maxfiles 软上限）、`process_darwin.go::getEnvironment`（`ps -E` 环境
//! 提取，SIP 受限时常为空）。

/// launchd label 输入校验（witr `isValidServiceLabel`）：只允许字母数字与
/// `. _ -`，长度 1-256（防命令注入；Runquiry 只经 argv 调用，此处保留
/// parity 校验语义）。
#[must_use]
pub(crate) fn is_valid_service_label(label: &str) -> bool {
    (1..=256).contains(&label.chars().count())
        && label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

/// launchd label 候选（witr `resolveLaunchdServicePID`）：name、
/// com.apple.name、org.name、io.name 四种。
#[must_use]
pub(crate) fn candidate_labels(name: &str) -> Vec<String> {
    vec![
        String::from(name),
        format!("com.apple.{name}"),
        format!("org.{name}"),
        format!("io.{name}"),
    ]
}

/// 解析 `launchctl print <domain>/<label>` 输出中的 `pid = <n>`（witr
/// `resolveLaunchdServicePID` 同语义）。
#[must_use]
pub(crate) fn parse_print_pid(output: &str) -> Option<u32> {
    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(raw) = trimmed.strip_prefix("pid = ") {
            if let Ok(pid) = raw.trim().parse::<u32>() {
                if pid > 0 {
                    return Some(pid);
                }
            }
        }
    }
    None
}

/// 解析 `launchctl blame <pid>` 输出：仅「真实服务路径」（含 `/`）返回
/// `(domain, label)`（witr `GetServiceLabel`）；`speculative` 等 blame 原因
/// 返回 `None`。`gui/501/label` 归并为 `gui/501` + label。
#[must_use]
pub(crate) fn parse_blame_service(output: &str) -> Option<(String, String)> {
    let line = output.trim();
    if line.is_empty() || !line.contains('/') {
        return None;
    }
    let (domain, label) = line.split_once('/')?;
    if domain == "gui" {
        let (uid, label) = label.split_once('/')?;
        return Some((format!("gui/{uid}"), label.to_string()));
    }
    Some((domain.to_string(), label.to_string()))
}

/// 从 `launchctl list` 输出按 PID 找 label（witr `findServiceByPID` 回退：
/// 行格式 `PID Status Label`）。
#[must_use]
pub(crate) fn parse_list_label(output: &str, pid: u32) -> Option<String> {
    for line in output.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 3 && fields[0].parse::<u32>().ok() == Some(pid) {
            return Some(fields[2].to_string());
        }
    }
    None
}

/// 解析 `launchctl limit maxfiles` 输出的软上限（witr
/// `parseLaunchctlLimitLine`）：含 `maxfiles` 行的第 2 列；`unlimited`
/// （不分大小写）记 0（witr 约定）；不可解析返回 `None`。
#[must_use]
pub(crate) fn parse_limit_maxfiles(output: &str) -> Option<u64> {
    for line in output.lines() {
        if !line.contains("maxfiles") {
            continue;
        }
        let soft = line.split_whitespace().nth(1)?;
        if soft.eq_ignore_ascii_case("unlimited") {
            return Some(0);
        }
        return soft.parse::<u64>().ok();
    }
    None
}

/// launchd 域描述（witr `DomainDescription`）。
#[must_use]
pub(crate) fn domain_description(domain: &str) -> String {
    if domain == "system" {
        String::from("Launch Daemon")
    } else if domain == "user" || domain.starts_with("gui/") {
        String::from("Launch Agent")
    } else {
        String::from("launchd service")
    }
}

/// launchd plist 搜索路径（witr `plistSearchPaths`，`~` 以 `$HOME` 展开）。
pub(crate) const PLIST_SEARCH_PATHS: [&str; 5] = [
    "~/Library/LaunchAgents",
    "/Library/LaunchAgents",
    "/Library/LaunchDaemons",
    "/System/Library/LaunchAgents",
    "/System/Library/LaunchDaemons",
];

/// plist 路径候选（witr `FindPlistPath`：搜索路径 + `<label>.plist`；
/// `~` 以 `$HOME` 展开，`$HOME` 未设时保留 `~` 原文——该候选必然落空）。
#[must_use]
pub(crate) fn plist_candidates(label: &str) -> Vec<String> {
    let home = std::env::var("HOME").unwrap_or_default();
    PLIST_SEARCH_PATHS
        .iter()
        .map(|path| {
            let expanded = path
                .strip_prefix("~/")
                .map_or_else(|| String::from(*path), |rest| format!("{home}/{rest}"));
            format!("{expanded}/{label}.plist")
        })
        .collect()
}

/// 从 `ps -p <pid> -E -o command=` 输出提取环境变量键值（witr
/// `getEnvironment`：SIP 受限时输出不可得或不含环境）。
///
/// 提取规则：token 含 `=` 且不以 `-` 开头；变量名只允许字母数字与 `_`。
/// 返回 `(键, 值)` 对（首个 `=` 切分）；**值不得写入任何日志**。
#[must_use]
pub(crate) fn parse_env_from_ps_command(output: &str) -> Vec<(String, String)> {
    output
        .split_whitespace()
        .filter(|token| token.contains('=') && !token.starts_with('-'))
        .filter_map(|token| {
            let (name, value) = token.split_once('=')?;
            (!name.is_empty() && is_env_var_name(name)).then(|| {
                (
                    name.to_string(),
                    token[name.len() + 1..].to_string(),
                )
            })
        })
        .collect()
}

/// 环境变量名形态校验（witr `isEnvVarName`：仅字母数字与 `_`）。
fn is_env_var_name(name: &str) -> bool {
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_validation_matches_witr_charset() {
        assert!(is_valid_service_label("com.example.fxt-app"));
        assert!(is_valid_service_label("a"));
        assert!(!is_valid_service_label(""));
        assert!(!is_valid_service_label("bad;label"));
        assert!(!is_valid_service_label(&"a".repeat(257)));
    }

    #[test]
    fn print_pid_parses_only_first_valid_line() {
        assert_eq!(parse_print_pid("state = running\npid = 5150\n"), Some(5150));
        assert_eq!(parse_print_pid("no pid here"), None);
        assert_eq!(parse_print_pid("pid = 0"), None);
    }

    #[test]
    fn blame_service_requires_slash_and_handles_gui() {
        assert_eq!(
            parse_blame_service("gui/501/com.apple.fxt-agent"),
            Some((String::from("gui/501"), String::from("com.apple.fxt-agent")))
        );
        assert_eq!(
            parse_blame_service("system/fxt-daemon"),
            Some((String::from("system"), String::from("fxt-daemon")))
        );
        assert_eq!(parse_blame_service("speculative"), None);
        assert_eq!(parse_blame_service(""), None);
    }

    #[test]
    fn limit_maxfiles_parses_soft_limit() {
        assert_eq!(parse_limit_maxfiles("maxfiles    256            unlimited"), Some(256));
        assert_eq!(parse_limit_maxfiles("maxfiles    unlimited      unlimited"), Some(0));
        assert_eq!(parse_limit_maxfiles(""), None);
    }

    #[test]
    fn env_extraction_skips_flags_and_invalid_names() {
        let pairs = parse_env_from_ps_command(
            "/bin/fxt-app -c PATH=/usr/bin -- HOME=/Users/fixture-user bad$=x",
        );
        assert_eq!(
            pairs,
            vec![
                (String::from("PATH"), String::from("/usr/bin")),
                (String::from("HOME"), String::from("/Users/fixture-user")),
            ]
        );
    }
}