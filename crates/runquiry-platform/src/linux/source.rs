//! Linux 来源证据采集：`SourceEvidenceProvider` 实现。
//!
//! 平台只采集原始证据（cgroup 原文、environ 键值、systemd 可用性与 D-Bus
//! 富化键值），**不做任何来源类型判定**（core 的 detect 职责）。环境变量值
//! 不写入任何日志或错误消息（采集即脱敏边界）。

use std::sync::{Arc, atomic::AtomicBool, atomic::Ordering};
use std::time::Duration;

use self::systemd::enrich_systemd;
use super::process::{LinuxPlatform, read_cgroup, read_environ};
use runquiry_core::{ProcessSummary, SourceEvidence, SourceEvidenceProvider};

mod systemd;

/// systemd D-Bus 交互上限（witr `dbusTimeout` 同值：挂死的总线不能拖住采集）。
const DBUS_TIMEOUT: Duration = Duration::from_secs(2);

impl SourceEvidenceProvider for LinuxPlatform {
    fn evidence(&self, ancestry: &[ProcessSummary]) -> SourceEvidence {
        let mut evidence = SourceEvidence::default();
        for process in ancestry {
            let pid = process.identity.pid().get();
            if let Some(cgroup) = read_cgroup(&self.procfs, pid) {
                evidence
                    .cgroup_by_pid
                    .push((process.identity.pid(), cgroup));
            }
            let env = read_environ(&self.procfs, pid);
            if !env.is_empty() {
                evidence.env_by_pid.push((process.identity.pid(), env));
            }
        }
        // parity：IsSystemdRunning 只看 /run/systemd/system 存在性。
        evidence.systemd_running = self.systemd_run_dir.exists();
        if evidence.systemd_running
            && let Some((_, target_cgroup)) = evidence.cgroup_by_pid.last()
            && let Some(unit) = runquiry_core::systemd_unit_from_cgroup(target_cgroup)
        {
            // best-effort 富化：总线缺失、权限不足或单元未加载只省略对应键。
            evidence.systemd_details =
                enrich_systemd_bounded(&unit, Arc::clone(&self.systemd_in_flight));
        }
        evidence
    }
}

/// 有界 D-Bus 富化：独立线程执行 + `recv_timeout` 兜底，总线挂死不拖住
/// 调用方（超时的线程随后自行结束并释放连接）。
fn enrich_systemd_bounded(unit: &str, in_flight: Arc<AtomicBool>) -> Vec<(String, String)> {
    let unit = String::from(unit);
    enrich_systemd_with(in_flight, DBUS_TIMEOUT, move || enrich_systemd(&unit))
}

fn enrich_systemd_with<F>(
    in_flight: Arc<AtomicBool>,
    timeout: Duration,
    work: F,
) -> Vec<(String, String)>
where
    F: FnOnce() -> Vec<(String, String)> + Send + 'static,
{
    if in_flight
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Vec::new();
    }
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _guard = InFlightGuard(in_flight);
        drop(sender.send(work()));
    });
    receiver.recv_timeout(timeout).unwrap_or_default()
}

struct InFlightGuard(Arc<AtomicBool>);

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicBool, mpsc};
    use std::time::{Duration, Instant};

    use super::enrich_systemd_with;

    #[test]
    fn systemd_enrichment_is_single_flight_without_sleep() -> Result<(), String> {
        let in_flight = Arc::new(AtomicBool::new(false));
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker_flag = Arc::clone(&in_flight);
        let worker = std::thread::spawn(move || {
            enrich_systemd_with(worker_flag, Duration::from_secs(1), move || {
                assert!(started_tx.send(()).is_ok());
                assert!(release_rx.recv().is_ok());
                vec![(String::from("Description"), String::from("fixture"))]
            })
        });
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .map_err(|error| error.to_string())?;

        let duplicate = enrich_systemd_with(Arc::clone(&in_flight), Duration::from_secs(1), || {
            vec![(String::from("unexpected"), String::from("worker"))]
        });
        assert!(duplicate.is_empty(), "重复刷新不得启动第二个 worker");
        release_tx.send(()).map_err(|error| error.to_string())?;
        let first = worker.join().map_err(|_| String::from("worker panic"))?;
        assert_eq!(first.len(), 1);
        Ok(())
    }

    #[test]
    fn systemd_enrichment_timeout_returns_before_worker_finishes() -> Result<(), String> {
        let in_flight = Arc::new(AtomicBool::new(false));
        let (release_tx, release_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let started = Instant::now();
        let details = enrich_systemd_with(in_flight, Duration::from_millis(10), move || {
            assert!(release_rx.recv().is_ok());
            assert!(done_tx.send(()).is_ok());
            Vec::new()
        });
        assert!(details.is_empty());
        assert!(started.elapsed() < Duration::from_secs(1));
        release_tx.send(()).map_err(|error| error.to_string())?;
        done_rx
            .recv_timeout(Duration::from_secs(1))
            .map_err(|error| error.to_string())?;
        Ok(())
    }
}
