//! 部分成功、错误与平台能力的展示状态映射。

use runquiry_core::{CapabilityStatus, DiagnosticCode, DiagnosticIssue, Inspection};

/// 数据表面状态，保留部分成功与能力边界。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SurfaceState {
    /// 数据采集进行中。
    Loading,
    /// 完整数据。
    Ready,
    /// CPU 尚无第二个采样点。
    Sampling,
    /// 成功但零行。
    Empty,
    /// 数据与诊断并存。
    Partial {
        /// 与可用数据共同返回的诊断数量。
        issue_count: usize,
    },
    /// 完全失败。
    Error {
        /// 完全失败时的诊断数量。
        issue_count: usize,
    },
    /// 权限边界。
    PermissionDenied,
    /// 平台不支持。
    Unsupported {
        /// 平台不支持原因。
        reason: String,
    },
    /// 平台能力暂不可用。
    Unavailable {
        /// 当前环境不可用原因。
        reason: String,
    },
}

impl SurfaceState {
    /// CPU 展示状态。
    pub const fn cpu(value: Option<f64>) -> Self {
        match value {
            Some(_) => Self::Ready,
            None => Self::Sampling,
        }
    }

    /// 映射平台能力，不用空数据伪装能力缺失。
    pub fn from_capability(capability: &CapabilityStatus) -> Self {
        match capability {
            CapabilityStatus::Supported => Self::Ready,
            CapabilityStatus::Partial(_) => Self::Partial { issue_count: 0 },
            CapabilityStatus::Unsupported(reason) => Self::Unsupported {
                reason: reason.clone(),
            },
            CapabilityStatus::Unavailable(reason) => Self::Unavailable {
                reason: reason.clone(),
            },
        }
    }
}

/// 状态映射保持对原始数据与诊断的借用，不丢失部分结果。
#[derive(Debug)]
pub struct SurfaceSnapshot<'a, T> {
    state: SurfaceState,
    data: Option<&'a [T]>,
    issues: &'a [DiagnosticIssue],
    capability_reason: Option<&'a str>,
}

impl<'a, T> SurfaceSnapshot<'a, T> {
    /// 从能力与采集结果建立展示快照。
    pub fn new(capability: &'a CapabilityStatus, inspection: &'a Inspection<Vec<T>>) -> Self {
        let boundary = SurfaceState::from_capability(capability);
        let state = match boundary {
            SurfaceState::Ready => state_from_inspection(inspection),
            SurfaceState::Partial { .. } => match inspection.data {
                Some(_) => SurfaceState::Partial {
                    issue_count: inspection.issues.len(),
                },
                None => state_from_inspection(inspection),
            },
            state @ (SurfaceState::Loading
            | SurfaceState::Sampling
            | SurfaceState::Empty
            | SurfaceState::Error { .. }
            | SurfaceState::PermissionDenied
            | SurfaceState::Unsupported { .. }
            | SurfaceState::Unavailable { .. }) => state,
        };
        Self {
            state,
            data: inspection.data.as_deref(),
            issues: &inspection.issues,
            capability_reason: capability.reason(),
        }
    }

    /// 映射后的语义状态。
    pub const fn state(&self) -> &SurfaceState {
        &self.state
    }

    /// 原始数据切片；部分成功时仍为 `Some`。
    pub const fn data(&self) -> Option<&'a [T]> {
        self.data
    }

    /// 未改写的结构化诊断；展示层可提供具体受限原因。
    pub const fn issues(&self) -> &'a [DiagnosticIssue] {
        self.issues
    }

    /// 能力受限、不支持或不可用的原始原因。
    pub const fn capability_reason(&self) -> Option<&'a str> {
        self.capability_reason
    }
}

fn state_from_inspection<T>(inspection: &Inspection<Vec<T>>) -> SurfaceState {
    match (&inspection.data, inspection.issues.is_empty()) {
        (Some(data), true) if data.is_empty() => SurfaceState::Empty,
        (Some(_), true) => SurfaceState::Ready,
        (Some(_), false) => SurfaceState::Partial {
            issue_count: inspection.issues.len(),
        },
        (None, false)
            if inspection
                .issues
                .iter()
                .all(|issue| issue.code() == DiagnosticCode::PermissionDenied) =>
        {
            SurfaceState::PermissionDenied
        }
        (None, _) => SurfaceState::Error {
            issue_count: inspection.issues.len(),
        },
    }
}
