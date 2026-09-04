#![cfg(unix)]
//! 生产 [`StdCommandRunner`] 的行为契约测试（真实子进程 + 假脚本，不 mock runner）。
//!
//! 覆盖：argv 逐元素原样传递（无 shell 解释痕迹）、程序缺失、非零退出、
//! 挂起超时终止与回收、stdout/stderr/双流超限截断、僵尸清理。

#[path = "container_support.rs"]
mod support;

use std::path::Path;
use std::time::Duration;

use runquiry_core::{
    CommandRunner, CommandSpec, InspectError, STDERR_LIMIT_BYTES, STDOUT_LIMIT_BYTES,
};
use runquiry_platform::command::{CommandFailure, StdCommandRunner};
use support::{TempDir, TestResult, hang_script_body, read_pid_file, wait_until_gone};

const TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn command_argv_elements_pass_through_verbatim_without_shell() -> TestResult {
    let dir = TempDir::new("argv")?;
    let script = dir.write_executable(
        "argv-dump",
        "#!/bin/sh\nfor arg in \"$@\"; do printf '<%s>' \"$arg\"; done\nprintf '\\n'\n",
    )?;
    let spec = CommandSpec::new(
        script.display().to_string(),
        [
            "hello world",
            "a\"b",
            "line1\nline2",
            "-x --flag",
            "",
            "'$HOME'",
            "$(touch runquiry-inject-marker)",
            "`id`",
            "a|b",
        ],
    );
    let output = StdCommandRunner.run(&spec, TIMEOUT)?;
    assert_eq!(output.exit_code, Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout,
        "<hello world><a\"b><line1\nline2><-x --flag><><'$HOME'>\
         <$(touch runquiry-inject-marker)><`id`><a|b>\n"
    );
    // 注入面：命令替换若被 shell 解释会产生副作用文件，这里必须原样传递。
    assert!(!Path::new("runquiry-inject-marker").exists());
    Ok(())
}

#[test]
fn command_missing_program_is_external_tool_and_classified_spawn() {
    let spec = CommandSpec::new("runquiry-b3-no-such-cli", ["--version"]);
    let result = StdCommandRunner.run(&spec, Duration::from_millis(500));
    assert!(
        matches!(
            result,
            Err(InspectError::ExternalTool { ref program, .. }) if program == "runquiry-b3-no-such-cli"
        ),
        "程序缺失必须是 ExternalTool，实际 {result:?}"
    );
    let classified = StdCommandRunner.run_classified(&spec, Duration::from_millis(500));
    assert!(matches!(classified, Err(CommandFailure::Spawn { .. })));
}

#[test]
fn command_nonzero_exit_returns_code_with_partial_stdout() -> TestResult {
    let dir = TempDir::new("exit3")?;
    let script =
        dir.write_executable("partial", "#!/bin/sh\nprintf '{\"containers\":['\nexit 3\n")?;
    let spec = CommandSpec::new(script.display().to_string(), ["unused"]);
    let output = StdCommandRunner.run(&spec, TIMEOUT)?;
    // 非零退出码不是 CommandRunner 层错误：原样返回给调用方判定。
    assert_eq!(output.exit_code, Some(3));
    assert!(String::from_utf8_lossy(&output.stdout).contains("{\"containers\":["));
    assert!(!output.stdout_truncated && !output.stderr_truncated);
    Ok(())
}

#[test]
fn command_hang_is_killed_on_timeout_without_leftover_process() -> TestResult {
    let dir = TempDir::new("hang")?;
    let pid_file = dir.path().join("pid");
    let script = dir.write_executable("hang", &hang_script_body(&pid_file, 30))?;
    let spec = CommandSpec::new(script.display().to_string(), ["unused"]);
    let result = StdCommandRunner.run_classified(&spec, Duration::from_millis(300));
    match result {
        Err(CommandFailure::Timeout { timeout, .. }) => {
            assert_eq!(timeout, Duration::from_millis(300));
        }
        other => return Err(format!("挂起命令必须返回 Timeout，实际 {other:?}").into()),
    }
    let pid = read_pid_file(&pid_file).ok_or("脚本未自报 PID")?;
    assert!(
        wait_until_gone(pid, Duration::from_secs(2)),
        "kill 后子进程未被回收"
    );
    Ok(())
}

#[test]
fn command_stdout_over_limit_is_truncated_and_process_reaped() -> TestResult {
    let dir = TempDir::new("stdout-limit")?;
    let pid_file = dir.path().join("pid");
    // 9 MiB stdout 后挂起：达到 8MiB 上限即终止，不留遗留进程。
    let body = format!(
        "#!/bin/sh\necho $$ > {}\nyes A | head -c 9437184\nexec sleep 30\n",
        pid_file.display()
    );
    let script = dir.write_executable("flood", &body)?;
    let spec = CommandSpec::new(script.display().to_string(), ["unused"]);
    let output = StdCommandRunner.run(&spec, Duration::from_secs(5))?;
    assert!(output.stdout_truncated, "stdout 超限必须标记截断");
    assert_eq!(output.stdout.len(), STDOUT_LIMIT_BYTES);
    assert!(!output.stderr_truncated);
    // 被终止的子进程退出码不可得。
    assert_eq!(output.exit_code, None);
    let pid = read_pid_file(&pid_file).ok_or("脚本未自报 PID")?;
    assert!(
        wait_until_gone(pid, Duration::from_secs(2)),
        "超限 kill 后子进程未被回收"
    );
    Ok(())
}

#[test]
fn command_stderr_over_limit_is_truncated_and_process_reaped() -> TestResult {
    let dir = TempDir::new("stderr-limit")?;
    let pid_file = dir.path().join("pid");
    let body = format!(
        "#!/bin/sh\necho $$ > {}\nyes B | head -c 9437184 1>&2\nexec sleep 30\n",
        pid_file.display()
    );
    let script = dir.write_executable("flood-err", &body)?;
    let spec = CommandSpec::new(script.display().to_string(), ["unused"]);
    let output = StdCommandRunner.run(&spec, Duration::from_secs(5))?;
    assert!(output.stderr_truncated, "stderr 超限必须标记截断");
    assert_eq!(output.stderr.len(), STDERR_LIMIT_BYTES);
    assert!(!output.stdout_truncated);
    let pid = read_pid_file(&pid_file).ok_or("脚本未自报 PID")?;
    assert!(
        wait_until_gone(pid, Duration::from_secs(2)),
        "超限 kill 后子进程未被回收"
    );
    Ok(())
}

#[test]
fn command_both_streams_over_limit_are_truncated_and_process_reaped() -> TestResult {
    let dir = TempDir::new("both-limit")?;
    let pid_file = dir.path().join("pid");
    // 两条流并发各写 12MiB：先到 8MiB 上限的流触发 kill，另一条流因继续排空
    // （读取线程不因截断停止）也已越过上限，双流都带截断标记。
    let body = format!(
        "#!/bin/sh\necho $$ > {}\nyes A | head -c 12582912 &\nyes B | head -c 12582912 1>&2 &\nwait\nexec sleep 30\n",
        pid_file.display()
    );
    let script = dir.write_executable("flood-both", &body)?;
    let spec = CommandSpec::new(script.display().to_string(), ["unused"]);
    let output = StdCommandRunner.run(&spec, Duration::from_secs(5))?;
    assert!(
        output.stdout_truncated && output.stderr_truncated,
        "双流超限必须都有截断标记"
    );
    assert_eq!(output.stdout.len(), STDOUT_LIMIT_BYTES);
    assert_eq!(output.stderr.len(), STDERR_LIMIT_BYTES);
    let pid = read_pid_file(&pid_file).ok_or("脚本未自报 PID")?;
    assert!(
        wait_until_gone(pid, Duration::from_secs(2)),
        "超限 kill 后子进程未被回收"
    );
    Ok(())
}

#[test]
fn command_normal_exit_leaves_no_zombie() -> TestResult {
    let dir = TempDir::new("clean-exit")?;
    let pid_file = dir.path().join("pid");
    let body = format!("#!/bin/sh\necho $$ > {}\necho done\n", pid_file.display());
    let script = dir.write_executable("quick", &body)?;
    let spec = CommandSpec::new(script.display().to_string(), ["unused"]);
    let output = StdCommandRunner.run(&spec, TIMEOUT)?;
    assert_eq!(output.exit_code, Some(0));
    let pid = read_pid_file(&pid_file).ok_or("脚本未自报 PID")?;
    // wait 已回收：/proc/<pid> 应消失（僵尸进程仍会保留目录）。
    assert!(
        wait_until_gone(pid, Duration::from_secs(2)),
        "正常退出后存在未回收进程"
    );
    Ok(())
}
