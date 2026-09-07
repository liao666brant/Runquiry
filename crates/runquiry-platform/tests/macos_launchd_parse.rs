//! C1 macOS launchctl / plist 解析测试（在 Linux 执行）。
//!
//! 原因：macOS 的 cfg 代码（OS 采集与 FFI）无法在本机编译，故仅通过
//! `#[path]` 引入 src/macos/ 下与 OS 无关的纯解析模块（launchctl、XML
//! plist 提取，只依赖 std）在 Linux 直接编译运行。
//!
//! 覆盖场景（C1 必测）：launchctl print 的 `pid = <n>` 解析、blame 服务
//! 路径解析、label 校验与候选、maxfiles 软上限；plist 缺失字段、嵌套
//! dict（KeepAlive dict 按 false）、实体转义、StartCalendarInterval 的
//! dict / 数组两种形态。全部数据为合成值。
// launchctl 的 plist 候选路径与 ps -E 环境提取仅被 cfg(macos) 生产模块
// 消费，本目标不含。
#![allow(missing_docs, dead_code)]

#[path = "../src/macos/launchctl.rs"]
mod launchctl;
#[path = "../src/macos/plist/mod.rs"]
mod plist;

use launchctl::{
    candidate_labels, domain_description, is_valid_service_label, parse_blame_service,
    parse_limit_maxfiles, parse_list_label, parse_print_pid,
};
use plist::{LaunchdPlistInfo, format_triggers, parse_plist_xml};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn print_pid_follows_witr_name_resolution_contract() -> TestResult {
    // witr resolveLaunchdServicePID：`pid = <n>` 行解析，pid > 0 才有效。
    assert_eq!(
        parse_print_pid("program = /bin/fxt-app\npid = 5150\nstate = running\n"),
        Some(5150)
    );
    assert_eq!(parse_print_pid("pid = 0"), None);
    assert_eq!(parse_print_pid("pid = -1"), None);
    Ok(())
}

#[test]
fn blame_service_paths_and_reasons() -> TestResult {
    assert_eq!(
        parse_blame_service("system/com.fxt.daemon"),
        Some((String::from("system"), String::from("com.fxt.daemon")))
    );
    assert_eq!(
        parse_blame_service("gui/501/com.fxt.agent"),
        Some((String::from("gui/501"), String::from("com.fxt.agent")))
    );
    // blame 原因（非服务路径）返回 None：core 回退基础 launchd 来源。
    assert_eq!(parse_blame_service("speculative"), None);
    Ok(())
}

#[test]
fn list_label_and_candidates_follow_witr() -> TestResult {
    assert_eq!(
        parse_list_label("5150\t0\tcom.fxt.daemon", 5150),
        Some(String::from("com.fxt.daemon"))
    );
    assert_eq!(
        candidate_labels("fxt"),
        vec![
            String::from("fxt"),
            String::from("com.apple.fxt"),
            String::from("org.fxt"),
            String::from("io.fxt"),
        ]
    );
    assert!(is_valid_service_label("com.fxt.app-1_2"));
    assert!(!is_valid_service_label("bad;rm"));
    Ok(())
}

#[test]
fn limit_maxfiles_and_domain_descriptions() -> TestResult {
    assert_eq!(
        parse_limit_maxfiles("maxfiles    256            unlimited"),
        Some(256)
    );
    assert_eq!(
        parse_limit_maxfiles("maxfiles    unlimited      unlimited"),
        Some(0)
    );
    assert_eq!(domain_description("system"), "Launch Daemon");
    assert_eq!(domain_description("gui/501"), "Launch Agent");
    assert_eq!(domain_description("user"), "Launch Agent");
    assert_eq!(domain_description("loginwindow"), "launchd service");
    Ok(())
}

#[test]
fn plist_extracts_witr_consumed_fields() -> TestResult {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.fxt.agent&amp;1</string>
    <key>Comment</key>
    <string>合成 fixture 服务 &lt;说明&gt;</string>
    <key>Program</key>
    <string>/Users/fixture-user/Library/RunquiryFixtures/bin/fxt-app</string>
    <key>ProgramArguments</key>
    <array>
        <string>/Users/fixture-user/Library/RunquiryFixtures/bin/fxt-app</string>
        <string>--flag</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StartInterval</key>
    <integer>7200</integer>
    <key>WatchPaths</key>
    <array><string>/Users/fixture-user/Library/RunquiryFixtures/watch</string></array>
    <key>QueueDirectories</key>
    <array><string>/Users/fixture-user/Library/RunquiryFixtures/queue</string></array>
</dict>
</plist>
"#;
    let info = parse_plist_xml(xml);
    assert_eq!(info.label, "com.fxt.agent&1");
    assert_eq!(info.comment, "合成 fixture 服务 <说明>");
    assert_eq!(
        info.program,
        "/Users/fixture-user/Library/RunquiryFixtures/bin/fxt-app"
    );
    assert_eq!(info.program_arguments.len(), 2);
    assert!(info.run_at_load);
    assert!(info.keep_alive);
    assert_eq!(info.start_interval, 7200);
    assert_eq!(
        info.watch_paths,
        vec!["/Users/fixture-user/Library/RunquiryFixtures/watch"]
    );
    assert_eq!(format_triggers(&info).len(), 4);
    Ok(())
}

#[test]
fn plist_calendar_interval_dict_and_array() -> TestResult {
    let dict_xml = "<plist><dict><key>Label</key><string>fxt</string>\
        <key>StartCalendarInterval</key><dict><key>Weekday</key><integer>1</integer>\
        <key>Hour</key><integer>9</integer><key>Minute</key><integer>30</integer></dict>\
        </dict></plist>";
    let info = parse_plist_xml(dict_xml);
    assert_eq!(info.start_calendar_interval, "Mon at 09:30");
    assert_eq!(
        format_triggers(&info),
        vec!["StartCalendarInterval (Mon at 09:30)"]
    );

    let array_xml = "<plist><dict><key>Label</key><string>fxt</string>\
        <key>StartCalendarInterval</key><array>\
        <dict><key>Hour</key><integer>7</integer></dict>\
        <dict><key>Day</key><integer>1</integer><key>Minute</key><integer>5</integer></dict>\
        </array></dict></plist>";
    let info = parse_plist_xml(array_xml);
    assert_eq!(info.start_calendar_interval, "at 07:00; day 1 at *:05");
    assert_eq!(format_triggers(&info).len(), 1);
    Ok(())
}

#[test]
fn plist_nested_dict_and_missing_fields_are_tolerated() -> TestResult {
    // KeepAlive 为 dict（witr：dictDepth > 1 清空 currentKey → 按 false）；
    // EnvironmentVariables 等嵌套 dict 内键不入结果。
    let xml = "<plist><dict>\
        <key>Label</key><string>fxt</string>\
        <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>\
        <key>EnvironmentVariables</key><dict><key>SECRET</key><string>value</string></dict>\
        </dict></plist>";
    let info = parse_plist_xml(xml);
    assert_eq!(info.label, "fxt");
    assert!(!info.keep_alive);
    assert!(info.program_arguments.is_empty());
    assert_eq!(info.comment, "");
    // 空输入 / 截断输入：返回空结果（调用方记诊断），不 panic。
    assert_eq!(parse_plist_xml(""), LaunchdPlistInfo::default());
    assert_eq!(
        parse_plist_xml("<plist><dict><key>Label</key><str"),
        LaunchdPlistInfo::default()
    );
    Ok(())
}
