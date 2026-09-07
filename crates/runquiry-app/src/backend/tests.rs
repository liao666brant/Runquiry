//! 生产 backend 的目标解析与错误映射回归测试；Linux 实机回归（B7 语义，
//! 依赖 `/proc` 与进程控制）见 `linux_only` 子模块（仅 Linux 编译）。

use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, mpsc};
use std::thread;
use std::time::SystemTime;

use runquiry_core::{
    CapabilityStatus, DiagnosticCode, DiagnosticIssue, InspectError, Inspection, Pid,
    ProcessAction, ProcessIdentity,
};
use runquiry_ui::backend::WorkspaceBackend;

use super::{PlatformBackend, UnavailableBackend};

#[cfg(target_os = "linux")]
mod linux_only;

fn record_maximum(maximum: &AtomicUsize, value: usize) {
    let mut observed = maximum.load(Ordering::Acquire);
    while value > observed {
        match maximum.compare_exchange(observed, value, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return,
            Err(current) => observed = current,
        }
    }
}

#[test]
fn serializes_concurrent_analysis_admission() -> Result<(), Box<dyn std::error::Error>> {
    let backend = Arc::new(PlatformBackend::new()?);
    let barrier = Arc::new(Barrier::new(3));
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let (entered_tx, entered_rx) = mpsc::channel();
    let mut releases = Vec::new();
    let mut workers = Vec::new();

    for index in 0..2 {
        let (release_tx, release_rx) = mpsc::channel();
        releases.push(release_tx);
        let backend = Arc::clone(&backend);
        let barrier = Arc::clone(&barrier);
        let active = Arc::clone(&active);
        let maximum = Arc::clone(&maximum);
        let entered = entered_tx.clone();
        workers.push(thread::spawn(move || {
            barrier.wait();
            backend.with_analysis_gate(|| {
                let concurrent = active.fetch_add(1, Ordering::AcqRel) + 1;
                record_maximum(&maximum, concurrent);
                entered.send(index).map_err(|_| InspectError::Unsupported {
                    reason: String::from("analysis gate probe receiver closed"),
                })?;
                release_rx.recv().map_err(|_| InspectError::Unsupported {
                    reason: String::from("analysis gate probe release closed"),
                })?;
                active.fetch_sub(1, Ordering::AcqRel);
                Ok(())
            })
        }));
    }
    drop(entered_tx);
    barrier.wait();

    for _ in 0..2 {
        let index = entered_rx.recv()?;
        releases[index].send(())?;
    }
    for worker in workers {
        worker
            .join()
            .map_err(|_| io::Error::other("analysis gate probe thread panicked"))??;
    }
    assert_eq!(maximum.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn forwards_a_process_action_failure_without_reclassification()
-> Result<(), Box<dyn std::error::Error>> {
    let backend = PlatformBackend::new()?;
    let impossible_pid = Pid::new(999_999_999)?;
    let identity = ProcessIdentity::new(impossible_pid, Some(SystemTime::UNIX_EPOCH), None);

    let result = backend.execute_process_action(&identity, ProcessAction::Terminate);

    // 平台语义差异：Linux 对不存在 PID 返回 NotFound；Windows 进程控制整体
    // Unsupported。测试目标是「平台错误原样上抛、不二次改写」，故按平台断言。
    #[cfg(target_os = "linux")]
    assert!(matches!(
        result,
        Err(InspectError::NotFound { subject }) if subject == "进程 999999999"
    ));
    #[cfg(not(target_os = "linux"))]
    assert!(
        matches!(result, Err(InspectError::Unsupported { .. })),
        "进程控制不可用平台的边界错误应原样上抛，实际 {result:?}"
    );
    Ok(())
}

#[test]
fn unavailable_backend_disables_process_actions_with_its_construction_reason() {
    let backend = UnavailableBackend::new("Linux 平台采集器不可用");
    let identity = ProcessIdentity::new(Pid::MIN, Some(SystemTime::UNIX_EPOCH), None);

    assert_eq!(
        backend.process_control_capability(),
        CapabilityStatus::Unavailable(String::from("Linux 平台采集器不可用")),
    );
    assert!(matches!(
        backend.execute_process_action(&identity, ProcessAction::Terminate),
        Err(InspectError::Unsupported { reason }) if reason == "Linux 平台采集器不可用"
    ));
}

/// 采集完全失败时，app 边界必须保留平台给出的结构化结论，不折叠为单一文案。
#[test]
fn failed_collection_maps_platform_diagnostics_to_structured_errors() {
    let unsupported = Inspection::<Vec<()>>::failed(vec![DiagnosticIssue::new(
        DiagnosticCode::Unsupported,
        String::from("Windows 平台不提供文件锁枚举（parity §10）"),
    )]);
    let error = PlatformBackend::failed_collection_error(
        &unsupported,
        String::from("文件 /x"),
        String::from("采集未返回数据"),
    );
    assert!(
        matches!(error, InspectError::Unsupported { .. }),
        "能力不支持应保持 Unsupported，实际 {error:?}"
    );
    let InspectError::Unsupported { reason } = error else {
        return;
    };
    assert_eq!(reason, "Windows 平台不提供文件锁枚举（parity §10）");

    let denied = Inspection::<Vec<()>>::failed(vec![DiagnosticIssue::new(
        DiagnosticCode::PermissionDenied,
        String::from("permission"),
    )]);
    let error = PlatformBackend::failed_collection_error(
        &denied,
        String::from("端口 443"),
        String::from("采集未返回数据"),
    );
    assert!(
        matches!(error, InspectError::PermissionDenied { .. }),
        "权限失败应保留 subject，实际 {error:?}"
    );
    let InspectError::PermissionDenied { subject } = error else {
        return;
    };
    assert_eq!(subject, "端口 443");

    let error = PlatformBackend::failed_collection_error(
        &Inspection::<Vec<()>>::failed(vec![DiagnosticIssue::new(
            DiagnosticCode::ExternalToolFailed,
            String::from("lsof exited 2"),
        )]),
        String::from("subject"),
        String::from("采集未返回数据"),
    );
    assert!(
        matches!(error, InspectError::Unsupported { .. }),
        "无能力标注的失败应折叠为 Unsupported 并保留诊断，实际 {error:?}"
    );
    let InspectError::Unsupported { reason } = error else {
        return;
    };
    assert_eq!(reason, "采集未返回数据：lsof exited 2");
}
