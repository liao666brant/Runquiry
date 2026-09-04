//! 祖先链解析契约。

use runquiry_core::{Pid, resolve_ancestry};

use crate::support::collectors::{captured_at, summary};

use super::TestResult;
/// 祖先链：root→target 顺序（完整链 1←3←5）。
#[test]
fn pipeline_ancestry_returns_root_to_target_order() -> TestResult {
    let now = captured_at();
    let mut reads = std::collections::HashMap::new();
    reads.insert(1, summary(1, Some(0), "systemd", None));
    reads.insert(3, summary(3, Some(1), "sshd", None));
    reads.insert(5, summary(5, Some(3), "bash", None));
    let read =
        move |pid: Pid| Ok::<_, runquiry_core::DiagnosticIssue>(reads.get(&pid.get()).cloned());
    let inspection = resolve_ancestry(Pid::new(5)?, now, &read)?;
    let chain = inspection
        .data()
        .ok_or_else(|| String::from("应有祖先链"))?;
    assert_eq!(chain.len(), 3);
    assert_eq!(chain[0].identity.pid(), Pid::new(1)?, "root 在前");
    assert_eq!(chain[2].identity.pid(), Pid::new(5)?, "target 在尾");
    // PID 1 到达即止。
    assert_eq!(chain[0].command, "systemd");
    Ok(())
}

/// 单跳失败就地截断（父进程读不到 → 链截为目标与已读到部分）；
/// 目标本身不可读 → NotFound（进程已退出）。
#[test]
fn pipeline_ancestry_truncates_on_partial_read_and_fails_when_target_missing() -> TestResult {
    let now = captured_at();
    let mut reads = std::collections::HashMap::new();
    reads.insert(5, summary(5, Some(3), "bash", None)); // 父进程 3 读不到
    let snapshot_reads = reads.clone();
    let read = move |pid: Pid| {
        Ok::<_, runquiry_core::DiagnosticIssue>(snapshot_reads.get(&pid.get()).cloned())
    };
    let inspection = resolve_ancestry(Pid::new(5)?, now, &read)?;
    let chain = inspection
        .data()
        .ok_or_else(|| String::from("部分成功应保留数据"))?;
    assert_eq!(chain.len(), 1, "单跳失败就地截断");
    assert_eq!(chain[0].identity.pid(), Pid::new(5)?);
    // 截断可区分（根计划完成标准）：清单缺失的祖先记一条诊断。
    assert_eq!(inspection.issues.len(), 1, "缺失祖先记诊断");
    assert_eq!(inspection.issues[0].code().code(), "unknown");

    // 权限类失败同样可区分：Err 通道的诊断原样进入 issues。
    let only_target = reads;
    let read = move |pid: Pid| {
        if pid.get() == 3 {
            Err(runquiry_core::DiagnosticIssue::new(
                runquiry_core::DiagnosticCode::PermissionDenied,
                String::from("合成场景：父进程不可读"),
            ))
        } else {
            Ok(only_target.get(&pid.get()).cloned())
        }
    };
    let inspection = resolve_ancestry(Pid::new(5)?, now, &read)?;
    assert_eq!(inspection.issues[0].code().code(), "permission_denied");

    let nothing: std::collections::HashMap<u32, runquiry_core::ProcessSummary> =
        std::collections::HashMap::new();
    let read =
        move |pid: Pid| Ok::<_, runquiry_core::DiagnosticIssue>(nothing.get(&pid.get()).cloned());
    let err = resolve_ancestry(Pid::new(5)?, now, &read)
        .err()
        .ok_or_else(|| String::from("目标不可读应失败"))?;
    assert_eq!(err.code(), "not_found", "进程已退出 → NotFound");
    Ok(())
}

/// 环检测：PPID 回指已访问 PID 时截断，不发散（witr seen map loop protection）。
#[test]
fn pipeline_ancestry_breaks_pid_cycles() -> TestResult {
    let now = captured_at();
    let mut reads = std::collections::HashMap::new();
    reads.insert(9, summary(9, Some(10), "a", None));
    reads.insert(10, summary(10, Some(9), "b", None));
    let read =
        move |pid: Pid| Ok::<_, runquiry_core::DiagnosticIssue>(reads.get(&pid.get()).cloned());
    let inspection = resolve_ancestry(Pid::new(9)?, now, &read)?;
    let chain = inspection
        .data()
        .ok_or_else(|| String::from("环应有截断结果"))?;
    assert_eq!(chain.len(), 2, "9→10 后 10 的父 9 已访问，截断");
    assert!(inspection.issues.is_empty(), "环截断是正常终止，不记诊断");
    Ok(())
}
