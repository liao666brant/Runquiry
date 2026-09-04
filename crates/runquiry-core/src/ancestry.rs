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
