//! Analysis 详情分区的只读展示模型。

use runquiry_core::{Analysis, Inspection};

/// 详情面板的固定分区。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetailSection {
    /// 目标概览。
    Overview,
    /// 祖先链与子进程。
    Ancestry,
    /// 启动来源。
    Source,
    /// 告警。
    Warnings,
    /// CPU、内存与 I/O。
    Resources,
    /// Socket。
    Sockets,
    /// 文件锁与打开文件。
    Files,
    /// 环境变量。
    Environment,
}

/// 分区及其数据量；不复制 Analysis 的具体数据。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisSections {
    sections: [(DetailSection, usize); 8],
    issue_count: usize,
}

impl AnalysisSections {
    /// 从分析采集结果建立分区摘要；部分成功诊断不会被丢弃。
    pub fn from_inspection(inspection: &Inspection<Analysis>) -> Option<Self> {
        let analysis = inspection.data.as_ref()?;
        let detail_count = usize::from(analysis.details.is_some());
        let environment_count = analysis
            .details
            .as_ref()
            .map_or(0, |details| details.environment.len());
        let file_count = analysis.file_locks.len()
            + analysis
                .details
                .as_ref()
                .map_or(0, |details| details.open_files.len());
        Some(Self {
            sections: [
                (DetailSection::Overview, 1),
                (
                    DetailSection::Ancestry,
                    analysis.ancestry.len() + analysis.children.len(),
                ),
                (DetailSection::Source, 1),
                (DetailSection::Warnings, analysis.warnings.len()),
                (DetailSection::Resources, detail_count),
                (DetailSection::Sockets, analysis.sockets.len()),
                (DetailSection::Files, file_count),
                (DetailSection::Environment, environment_count),
            ],
            issue_count: inspection.issues.len(),
        })
    }

    /// 固定顺序的详情分区与条目数。
    pub const fn sections(&self) -> &[(DetailSection, usize); 8] {
        &self.sections
    }

    /// 与详情共同呈现的诊断数量。
    pub const fn issue_count(&self) -> usize {
        self.issue_count
    }
}
