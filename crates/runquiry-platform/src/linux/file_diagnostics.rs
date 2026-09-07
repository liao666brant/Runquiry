//! 文件清单采集的有界分类诊断。

use std::io::ErrorKind;

use runquiry_core::{DiagnosticCode, DiagnosticIssue};

#[derive(Debug, Clone, Copy)]
pub(super) enum FileIoStage {
    LockTable,
    Comm,
    FdDirectory,
    FdLink,
}

#[derive(Debug, Clone, Copy)]
enum FileIssueKind {
    LockTableDenied,
    LockTableOther,
    CommDenied,
    CommOther,
    FdDirectoryDenied,
    FdDirectoryOther,
    FdLinkDenied,
    FdLinkOther,
    InvalidPid,
    InvalidLockMode,
}

impl FileIssueKind {
    const ALL: [Self; 10] = [
        Self::LockTableDenied,
        Self::LockTableOther,
        Self::CommDenied,
        Self::CommOther,
        Self::FdDirectoryDenied,
        Self::FdDirectoryOther,
        Self::FdLinkDenied,
        Self::FdLinkOther,
        Self::InvalidPid,
        Self::InvalidLockMode,
    ];

    const fn index(self) -> usize {
        match self {
            Self::LockTableDenied => 0,
            Self::LockTableOther => 1,
            Self::CommDenied => 2,
            Self::CommOther => 3,
            Self::FdDirectoryDenied => 4,
            Self::FdDirectoryOther => 5,
            Self::FdLinkDenied => 6,
            Self::FdLinkOther => 7,
            Self::InvalidPid => 8,
            Self::InvalidLockMode => 9,
        }
    }

    const fn code(self) -> DiagnosticCode {
        match self {
            Self::LockTableDenied
            | Self::CommDenied
            | Self::FdDirectoryDenied
            | Self::FdLinkDenied => DiagnosticCode::PermissionDenied,
            Self::InvalidPid | Self::InvalidLockMode => DiagnosticCode::ParseFailed,
            Self::LockTableOther | Self::CommOther | Self::FdDirectoryOther | Self::FdLinkOther => {
                DiagnosticCode::Unknown
            }
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::LockTableDenied | Self::LockTableOther => "/proc/locks 读取失败",
            Self::CommDenied | Self::CommOther => "进程 comm 读取失败",
            Self::FdDirectoryDenied | Self::FdDirectoryOther => "进程 fd 目录读取失败",
            Self::FdLinkDenied | Self::FdLinkOther => "进程 fd 链接读取失败",
            Self::InvalidPid => "/proc/locks PID 非法",
            Self::InvalidLockMode => "/proc/locks 锁模式不可识别",
        }
    }
}

#[derive(Debug, Default)]
struct FileIssueBucket {
    count: usize,
    sample: Option<String>,
}

#[derive(Debug, Default)]
pub(super) struct FileDiagnostics {
    buckets: [FileIssueBucket; 10],
}

impl FileDiagnostics {
    fn record(&mut self, kind: FileIssueKind, sample: &str) {
        let bucket = &mut self.buckets[kind.index()];
        bucket.count = bucket.count.saturating_add(1);
        if bucket.sample.is_none() {
            bucket.sample = Some(sample.to_string());
        }
    }

    pub(super) fn record_invalid_pid(&mut self, pid: u32) {
        self.record(FileIssueKind::InvalidPid, &pid.to_string());
    }

    pub(super) fn record_invalid_lock_mode(&mut self, mode: &str) {
        self.record(FileIssueKind::InvalidLockMode, mode);
    }

    pub(super) fn record_io(&mut self, stage: FileIoStage, sample: &str, error: &std::io::Error) {
        let kind = match (stage, error.kind() == ErrorKind::PermissionDenied) {
            (FileIoStage::LockTable, true) => FileIssueKind::LockTableDenied,
            (FileIoStage::LockTable, false) => FileIssueKind::LockTableOther,
            (FileIoStage::Comm, true) => FileIssueKind::CommDenied,
            (FileIoStage::Comm, false) => FileIssueKind::CommOther,
            (FileIoStage::FdDirectory, true) => FileIssueKind::FdDirectoryDenied,
            (FileIoStage::FdDirectory, false) => FileIssueKind::FdDirectoryOther,
            (FileIoStage::FdLink, true) => FileIssueKind::FdLinkDenied,
            (FileIoStage::FdLink, false) => FileIssueKind::FdLinkOther,
        };
        self.record(kind, &format!("{sample}：{error}"));
    }

    pub(super) fn into_issues(self) -> Vec<DiagnosticIssue> {
        FileIssueKind::ALL
            .into_iter()
            .filter_map(|kind| {
                let bucket = &self.buckets[kind.index()];
                bucket.sample.as_ref().map(|sample| {
                    DiagnosticIssue::new(
                        kind.code(),
                        format!("{}：共 {} 项；示例：{sample}", kind.label(), bucket.count),
                    )
                })
            })
            .collect()
    }
}
