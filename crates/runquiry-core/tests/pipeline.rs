//! 分析管线集成测试入口；职责测试位于同名目录。

mod support;

pub(crate) type TestResult = Result<(), Box<dyn std::error::Error>>;

/// 64 位十六进制长 ID（合成值，非真实容器 ID）。
pub(crate) const LONG_HEX: &str =
    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[path = "pipeline/analysis_collectors.rs"]
mod analysis_collectors;
#[path = "pipeline/analysis_sources.rs"]
mod analysis_sources;
#[path = "pipeline/ancestry.rs"]
mod ancestry;
#[path = "pipeline/cgroup.rs"]
mod cgroup;
#[path = "pipeline/models.rs"]
mod models;
#[path = "pipeline/source_detection.rs"]
mod source_detection;
