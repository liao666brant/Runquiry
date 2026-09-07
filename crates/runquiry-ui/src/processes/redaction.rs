//! 详情会话内的环境变量与命令参数脱敏。

use std::borrow::Cow;

const MASK: &str = "••••••••";

/// 一个环境变量的展示值。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedactedEnvironment<'a> {
    /// 环境变量名。
    pub key: &'a str,
    value: Cow<'a, str>,
}

impl RedactedEnvironment<'_> {
    /// 当前允许展示的值。
    pub fn value(&self) -> &str {
        &self.value
    }
}

/// 一个命令行参数的展示值。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedactedArgument<'a> {
    value: Cow<'a, str>,
    secret: bool,
}

impl<'a> RedactedArgument<'a> {
    /// 解析单个 `--key=value` 参数并默认遮罩敏感值。
    pub fn parse(argument: &'a str) -> Self {
        let Some((key, _)) = argument.split_once('=') else {
            return Self {
                value: Cow::Borrowed(argument),
                secret: false,
            };
        };
        if is_sensitive(key) {
            Self {
                value: Cow::Owned(format!("{key}={MASK}")),
                secret: true,
            }
        } else {
            Self {
                value: Cow::Borrowed(argument),
                secret: false,
            }
        }
    }

    /// 参数是否包含敏感信息。
    pub const fn is_secret(&self) -> bool {
        self.secret
    }

    /// 当前展示文本。
    pub fn value(&self) -> &str {
        &self.value
    }
}

/// Reveal 只存活于当前进程详情会话。
#[derive(Clone, Debug, Default)]
pub struct DetailPrivacySession {
    revealed: bool,
}

impl DetailPrivacySession {
    /// 建立默认隐藏敏感值的详情会话。
    pub const fn new() -> Self {
        Self { revealed: false }
    }

    /// 当前会话是否已显式展示敏感值。
    pub const fn is_revealed(&self) -> bool {
        self.revealed
    }

    /// 用户在当前详情会话中显式请求展示。
    pub const fn reveal(&mut self) {
        self.revealed = true;
    }

    /// 刷新、选择或身份变化时清除展示授权。
    pub const fn reset(&mut self) {
        self.revealed = false;
    }

    /// 生成环境变量展示行，不改写领域数据。
    pub fn environment<'a>(&self, entries: &'a [(String, String)]) -> Vec<RedactedEnvironment<'a>> {
        entries
            .iter()
            .map(|(key, value)| RedactedEnvironment {
                key,
                value: if self.revealed || !is_sensitive(key) {
                    Cow::Borrowed(value)
                } else {
                    Cow::Borrowed(MASK)
                },
            })
            .collect()
    }

    /// 生成命令行参数展示行；敏感长选项的下一项同样被遮罩。
    ///
    /// 未 reveal 时不尝试重建 shell quoting：一旦遇到敏感选项，从它的值开始
    /// 截断剩余载荷，避免带空格的引号值被拆分后泄露尾段。
    pub fn command_line<'a>(&self, command_line: &'a str) -> Vec<RedactedArgument<'a>> {
        if self.revealed {
            return command_line
                .split_whitespace()
                .map(|argument| RedactedArgument {
                    value: Cow::Borrowed(argument),
                    secret: argument
                        .split_once('=')
                        .is_some_and(|(key, _)| is_sensitive(key))
                        || (argument.starts_with('-') && is_sensitive(argument)),
                })
                .collect();
        }

        let mut redacted = Vec::new();
        for argument in command_line.split_whitespace() {
            let assignment = argument.split_once('=');
            if assignment.is_some_and(|(key, _)| is_sensitive(key)) {
                redacted.push(RedactedArgument::parse(argument));
                break;
            }
            if argument.starts_with('-') && is_sensitive(argument) {
                redacted.push(RedactedArgument::parse(argument));
                redacted.push(RedactedArgument {
                    value: Cow::Borrowed(MASK),
                    secret: true,
                });
                break;
            }
            redacted.push(RedactedArgument::parse(argument));
        }
        redacted
    }
}

fn is_sensitive(key: &str) -> bool {
    let normalized = key
        .trim_start_matches('-')
        .replace(['-', '.'], "_")
        .to_ascii_uppercase();
    [
        "TOKEN",
        "SECRET",
        "PASSWORD",
        "PASSWD",
        "API_KEY",
        "PRIVATE_KEY",
        "AUTH",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}
