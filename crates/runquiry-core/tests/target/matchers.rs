//! 名称 token 与模糊匹配契约。

use runquiry_core::{matches_exact_token, matches_fuzzy};
#[test]
fn target_exact_token_follows_witr_eight_cases() {
    // 完整参数命中可执行名。
    assert!(matches_exact_token("nginx", "/usr/bin/nginx -g daemon off"));
    // 路径中间段命中（snap 模式：core24 匹配 /snap/core24/1349/bin/python）。
    assert!(matches_exact_token(
        "core24",
        "/snap/core24/1349/bin/python"
    ));
    // token 内子串不命中：foo 不匹配 /usr/local/bin/foo-bar。
    assert!(!matches_exact_token("foo", "/usr/local/bin/foo-bar"));
    // 裸参数命中。
    assert!(matches_exact_token("node", "node server.js production"));
    // 反斜杠路径段命中（Windows 风格）。
    assert!(matches_exact_token("App", r"C:\Program Files\App\bin.exe"));
    // 部分文件名不命中：python 不匹配 /usr/bin/python3。
    assert!(!matches_exact_token("python", "/usr/bin/python3"));
    // 带扩展名的文件基名命中。
    assert!(matches_exact_token(
        "witr.exe",
        r"C:\Program Files\App\witr.exe"
    ));
    // 空命令行永不命中。
    assert!(!matches_exact_token("nginx", ""));
}

/// 函数本身大小写敏感；witr 的 exact 比较由调用方先把两侧统一转小写
/// （`name_*.go` 均先 `ToLower` 再传入 matchesExactToken）。
#[test]
fn target_exact_token_is_case_sensitive_and_callers_lowercase() {
    assert!(!matches_exact_token("Nginx", "/usr/bin/nginx"));
    assert!(matches_exact_token(
        "nginx",
        "/usr/bin/NGINX".to_lowercase().as_str()
    ));
}

/// fuzzy 匹配：大小写不敏感子串（parity：exact=false 的名称匹配）。
#[test]
fn target_fuzzy_match_is_case_insensitive_substring() {
    assert!(matches_fuzzy("NGINX", "/usr/sbin/nginx: master process"));
    assert!(matches_fuzzy("nginx", "NGINX WORKER"));
    assert!(matches_fuzzy("Foo", "barfoobar"));
    assert!(!matches_fuzzy("nginx", "postgres"));
}
