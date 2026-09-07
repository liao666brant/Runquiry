//! 后端分析的跨刷新单飞门控。

use std::sync::Mutex;

use runquiry_core::InspectError;

use super::PlatformBackend;

/// 跨 fresh Linux 平台实例保留的分析互斥门。
#[derive(Debug)]
pub(super) struct AnalysisGate(Mutex<()>);

impl AnalysisGate {
    /// 建立未占用门控。
    pub(super) const fn new() -> Self {
        Self(Mutex::new(()))
    }

    fn run<T>(&self, work: impl FnOnce() -> Result<T, InspectError>) -> Result<T, InspectError> {
        let _guard = self.0.lock().map_err(|_| InspectError::Unsupported {
            reason: String::from("分析并发门控已中毒"),
        })?;
        work()
    }
}

impl PlatformBackend {
    /// 在同一应用后端内串行化完整分析，避免多个 fresh 平台同时富化 systemd。
    pub(super) fn with_analysis_gate<T>(
        &self,
        work: impl FnOnce() -> Result<T, InspectError>,
    ) -> Result<T, InspectError> {
        self.analysis_gate.run(work)
    }
}
