//! plist XML 的标签流 → 事件流切分与文本实体还原（纯逻辑）。

/// plist 标签流事件（`key` / `string` / `integer` 携带闭标签内的文本）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Event {
    /// `<dict>` 开标签。
    Dict,
    /// `</dict>` 闭标签。
    EndDict,
    /// `<array>` 开标签。
    Array,
    /// `</array>` 闭标签。
    EndArray,
    /// `<key>…</key>` 的文本。
    Key(String),
    /// `<string>…</string>` 的文本。
    Str(String),
    /// `<integer>…</integer>` 的原始文本。
    Int(String),
    /// `<true/>`。
    True,
    /// `<false/>`。
    False,
}

/// 把标签流切成事件：dict/array 的开闭、true/false 单目事件；key /
/// string / integer 在开标签处捕获到对应闭标签的文本。注释与未识别标签
/// 跳过。
pub(super) fn tokenize(data: &str) -> Vec<Event> {
    let mut events = Vec::new();
    let mut idx = 0;
    while let Some(offset) = data[idx..].find('<') {
        let start = idx + offset;
        let Some(relative_end) = data[start..].find('>') else {
            break;
        };
        let raw_tag = &data[start + 1..start + relative_end];
        let after_tag = start + relative_end + 1;
        if let Some(name) = raw_tag.strip_prefix('/') {
            // 闭合标签：文本元素已在开标签分支捕获，这里只处理容器。
            match name.trim() {
                "dict" => events.push(Event::EndDict),
                "array" => events.push(Event::EndArray),
                _ => {}
            }
            idx = after_tag;
        } else if raw_tag.starts_with("!--") {
            // 注释：跳到 `-->`。
            match data[after_tag..].find("-->") {
                Some(end) => idx = after_tag + end + 3,
                None => break,
            }
        } else {
            let name = raw_tag.trim_end_matches('/').trim();
            match name {
                "dict" => events.push(Event::Dict),
                "array" => events.push(Event::Array),
                "true" => events.push(Event::True),
                "false" => events.push(Event::False),
                "key" | "string" | "integer" => {
                    let close = format!("</{name}>");
                    match data[after_tag..].find(&close) {
                        Some(close_at) => {
                            let text = decode_entities(&data[after_tag..after_tag + close_at]);
                            events.push(match name {
                                "key" => Event::Key(text),
                                "string" => Event::Str(text),
                                _ => Event::Int(text),
                            });
                            idx = after_tag + close_at + close.len();
                        }
                        // 未闭合的文本元素：按损坏输入停止扫描。
                        None => break,
                    }
                }
                _ => idx = after_tag,
            }
        }
    }
    events
}

/// XML 实体还原（标准五实体 + 十六进制/十进制数字引用）。
#[must_use]
fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return String::from(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        let Some(end) = tail.find(';') else {
            out.push('&');
            rest = &rest[pos + 1..];
            continue;
        };
        let entity = &tail[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| {
                    entity
                        .strip_prefix('#')
                        .and_then(|dec| dec.parse::<u32>().ok())
                })
                .and_then(char::from_u32),
        };
        match decoded {
            Some(ch) => {
                out.push(ch);
                rest = &tail[end + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}
