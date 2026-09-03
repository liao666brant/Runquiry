//! 部分成功容器 `Inspection<T>`。

use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::model::diagnostic::DiagnosticIssue;

/// 一次采集的结果快照：数据与诊断并存，支持部分成功。
///
/// 语义来自上级方案「单个权限或工具错误不得丢弃其余数据」：
/// * `data = Some(..)` 且 `issues` 非空 —— 部分成功，UI 应同时呈现数据与受限说明；
/// * `data = None` —— 完全失败，`issues` 说明原因；
/// * `data = Some(..)` 且 `issues` 为空 —— 完整成功。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inspection<T> {
    /// 取得的数据；完全失败时为 `None`。
    pub data: Option<T>,
    /// 结构化诊断列表；单个 issue 不得抹掉已取得数据。
    pub issues: Vec<DiagnosticIssue>,
    /// 快照采集时刻。
    pub captured_at: SystemTime,
}

impl<T> Inspection<T> {
    /// 完整成功：只有数据，没有诊断。
    pub fn complete(data: T) -> Self {
        Self {
            data: Some(data),
            issues: Vec::new(),
            captured_at: SystemTime::now(),
        }
    }

    /// 部分成功：数据与诊断并存。
    pub fn partial(data: T, issues: Vec<DiagnosticIssue>) -> Self {
        Self {
            data: Some(data),
            issues,
            captured_at: SystemTime::now(),
        }
    }

    /// 完全失败：没有数据，只有诊断。
    pub fn failed(issues: Vec<DiagnosticIssue>) -> Self {
        Self {
            data: None,
            issues,
            captured_at: SystemTime::now(),
        }
    }

    /// 以显式采集时刻构造（供 fixture 固定行为）。
    pub const fn with_captured_at(
        data: Option<T>,
        issues: Vec<DiagnosticIssue>,
        captured_at: SystemTime,
    ) -> Self {
        Self {
            data,
            issues,
            captured_at,
        }
    }

    /// 是否完全没有取得数据。
    pub const fn is_empty(&self) -> bool {
        self.data.is_none()
    }

    /// 是否存在诊断（即是否为部分成功或完全失败）。
    pub const fn has_issues(&self) -> bool {
        !self.issues.is_empty()
    }

    /// 追加一条诊断（平台实现逐条记录单点失败时使用）。
    pub fn push_issue(&mut self, issue: DiagnosticIssue) {
        self.issues.push(issue);
    }

    /// 对已取得的数据做映射，保留诊断与采集时刻。
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Inspection<U> {
        Inspection {
            data: self.data.map(f),
            issues: self.issues,
            captured_at: self.captured_at,
        }
    }

    /// 借用已取得的数据。
    pub const fn data(&self) -> Option<&T> {
        self.data.as_ref()
    }
}
