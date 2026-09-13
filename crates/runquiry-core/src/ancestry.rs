//! 祖先链解析（parity §4：witr `internal/proc/ancestry.go` 的 `ResolveAncestry`）。
//!
//! 从目标 PID 沿 PPID 逐跳上溯：单跳失败就地截断（部分结果；失败原因经
//! [`AncestryReader`] 的错误通道回传并写入 `Inspection.issues`，使截断链与
//! 完整链可区分——根计划完成标准「部分结果可区分」）；已访问集合做环保护；
//! 到达 PPID 不可得或 PID 1 终止；最终反转为 root→target 顺序返回。
//! 一跳都没读到（目标本身不可读）→ [`InspectError::NotFound`]（进程已退出）。

use std::time::SystemTime;

use crate::model::diagnostic::{DiagnosticCode, DiagnosticIssue};
use crate::model::error::InspectError;
use crate::model::ids::Pid;
use crate::model::inspection::Inspection;
use crate::model::process::ProcessSummary;

/// 单跳进程读取函数：
/// * `Ok(Some(summary))` —— 读到，链继续；
/// * `Ok(None)` —— 快照成功但该 PID 不在其中（采集间隙消失/清单缺失），
///   链就地截断并记一条诊断；
/// * `Err(issue)` —— 读取失败（权限不足、快照整体失败等），链就地截断并
///   原样记录该诊断。
pub type AncestryReader<'a> = dyn Fn(Pid) -> Result<Option<ProcessSummary>, DiagnosticIssue> + 'a;

/// 解析目标进程的祖先链（root→target 顺序）。
///
/// 读取函数由调用方注入（平台/管线提供；core 不读文件系统）。环保护对齐
/// witr：PPID 回指已访问 PID 时直接截断而非报错。
///
/// # Errors
/// 目标本身不可读（一跳都没读到）时返回 [`InspectError::NotFound`]；此时
/// 截断诊断无法经 `Inspection` 携带，由调用方从清单采集诊断另行获得。
pub fn resolve_ancestry(
    target: Pid,
    now: SystemTime,
    read: &AncestryReader<'_>,
) -> Result<Inspection<Vec<ProcessSummary>>, InspectError> {
    let mut chain: Vec<ProcessSummary> = Vec::new();
    let mut issues: Vec<DiagnosticIssue> = Vec::new();
    let mut seen: Vec<Pid> = Vec::new();
    let mut current = Some(target);

    while let Some(pid) = current {
        if seen.contains(&pid) {
            break; // 环保护：截断（parity：seen map loop protection）
        }
        seen.push(pid);
        let summary = match read(pid) {
            Ok(Some(summary)) => summary,
            Ok(None) => {
                issues.push(DiagnosticIssue::new(
                    DiagnosticCode::Unknown,
                    format!("祖先 PID {pid} 在进程清单中缺失（可能已退出），祖先链就此截断"),
                ));
                break;
            }
            Err(issue) => {
                issues.push(issue);
                break;
            }
        };
        let parent = summary.parent_pid;
        let self_pid = summary.identity.pid();
        chain.push(summary);
        if parent.is_none() || self_pid == Pid::MIN {
            break; // PPID 不可得或到达 PID 1：正常终止，不记诊断
        }
        current = parent;
    }

    if chain.is_empty() {
        return Err(InspectError::NotFound {
            subject: format!("PID {target}（进程可能已退出）"),
        });
    }
    chain.reverse(); // root→target
    Ok(Inspection::with_captured_at(Some(chain), issues, now))
}

/// 收集目标进程的全部后代（KillTree 用；快照内的多级 PPID 闭包）。
///
/// 广度优先逐层展开：直接子进程先于孙进程（与 KillTree「目标先死、后代
/// 随后按层杀」的顺序一致）；已访问集合做环防护（构造出的 PPID 环不致死
/// 循环）；按发现顺序返回，不含目标自身。
pub fn collect_descendants(target: Pid, snapshot: &[ProcessSummary]) -> Vec<ProcessSummary> {
    use std::collections::VecDeque;

    let mut descendants: Vec<ProcessSummary> = Vec::new();
    let mut seen = vec![target];
    let mut frontier: VecDeque<Pid> = VecDeque::from([target]);
    while let Some(pid) = frontier.pop_front() {
        for candidate in snapshot {
            let Some(parent) = candidate.parent_pid else {
                continue;
            };
            if parent != pid || seen.contains(&candidate.identity.pid()) {
                continue;
            }
            seen.push(candidate.identity.pid());
            descendants.push(candidate.clone());
            frontier.push_back(candidate.identity.pid());
        }
    }
    descendants
}

#[cfg(test)]
mod tests {
    use super::collect_descendants;
    use crate::model::health::HealthStatus;
    use crate::model::ids::Pid;
    use crate::model::process::ProcessSummary;

    fn summary(pid: u32, parent: Option<u32>) -> ProcessSummary {
        ProcessSummary {
            identity: crate::model::process::ProcessIdentity::new(
                Pid::new(pid).unwrap_or(Pid::MIN),
                None,
                None,
            ),
            parent_pid: parent.map(|raw| Pid::new(raw).unwrap_or(Pid::MIN)),
            command: format!("proc-{pid}"),
            command_line: None,
            user: None,
            health: HealthStatus::Unknown,
            container: None,
            exe_deleted: false,
            capabilities: Vec::new(),
            cpu_time_seconds: None,
            cpu_percent: None,
            memory_rss_bytes: None,
            memory_percent: None,
        }
    }

    #[test]
    fn descendants_are_collected_breadth_first_without_self() {
        // 树：1 → {2, 3}；2 → {4}；4 → {5}。期望 2, 3, 4, 5（同层相邻）。
        let snapshot = vec![
            summary(1, None),
            summary(2, Some(1)),
            summary(3, Some(1)),
            summary(4, Some(2)),
            summary(5, Some(4)),
        ];

        let collected = collect_descendants(Pid::new(1).unwrap_or(Pid::MIN), &snapshot);

        let pids: Vec<_> = collected.iter().map(|p| p.identity.pid().get()).collect();
        assert_eq!(pids, vec![2, 3, 4, 5]);
    }

    #[test]
    fn parent_pointing_back_at_descendant_does_not_loop() {
        // 构造 PPID 环：6 → 7 → 6。
        let snapshot = vec![summary(6, Some(7)), summary(7, Some(6))];

        let collected = collect_descendants(Pid::new(6).unwrap_or(Pid::MIN), &snapshot);

        let pids: Vec<_> = collected.iter().map(|p| p.identity.pid().get()).collect();
        assert_eq!(pids, vec![7]);
    }

    #[test]
    fn processes_without_parent_field_are_ignored() {
        let snapshot = vec![summary(8, None), summary(9, Some(8))];

        let collected = collect_descendants(Pid::new(42).unwrap_or(Pid::MIN), &snapshot);

        assert!(collected.is_empty());
    }
}
