//! 显式调查目标输入边界。

use runquiry_core::{
    InspectError, ProcessIdentity, QueryTarget, Resolution, parse_file_path, parse_pid, parse_port,
    parse_query,
};

/// 调查入口显式选择的目标类型；输入内容不会反向猜测类型。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetKind {
    /// 进程名称。
    Name,
    /// 进程 ID。
    Pid,
    /// 端口。
    Port,
    /// 文件路径。
    File,
    /// 容器。
    Container,
}

impl TargetKind {
    /// 在 UI 边界把文本解析为领域目标。
    ///
    /// # Errors
    /// 输入为空、PID 或端口越界时返回领域层的类型化错误。
    pub fn parse(self, raw: &str, exact: bool) -> Result<QueryTarget, InspectError> {
        match self {
            Self::Name => Ok(QueryTarget::ProcessName {
                query: parse_query(raw)?,
                exact,
            }),
            Self::Pid => Ok(QueryTarget::Pid(parse_pid(raw)?)),
            Self::Port => Ok(QueryTarget::Port(parse_port(raw)?)),
            Self::File => Ok(QueryTarget::File(parse_file_path(raw)?)),
            Self::Container => Ok(QueryTarget::Container {
                query: parse_query(raw)?,
                exact,
            }),
        }
    }
}

/// 调查解析在 UI 中的候选状态。
///
/// 歧义结果没有隐式选中项，必须由用户从候选表明确选择。
#[derive(Debug, Clone)]
pub enum QueryOutcome<T = ProcessIdentity> {
    /// 未命中。
    Empty,
    /// 唯一命中。
    Unique(T),
    /// 多候选。
    Ambiguous(Vec<T>),
}

impl<T> QueryOutcome<T> {
    /// 从 core 解析结果建立 UI 状态。
    pub fn from_resolution(resolution: Resolution<T>) -> Self {
        match resolution {
            Resolution::Unique(identity) => Self::Unique(identity),
            Resolution::Ambiguous(candidates) => Self::Ambiguous(candidates),
        }
    }

    /// 建立零命中状态。
    pub const fn empty() -> Self {
        Self::Empty
    }

    /// 候选集合；零命中为空。
    pub fn candidates(&self) -> &[T] {
        match self {
            Self::Empty => &[],
            Self::Unique(identity) => std::slice::from_ref(identity),
            Self::Ambiguous(candidates) => candidates,
        }
    }

    /// 仅唯一命中返回选择；歧义结果绝不默认选择首项。
    pub const fn selected(&self) -> Option<&T> {
        match self {
            Self::Unique(identity) => Some(identity),
            Self::Empty | Self::Ambiguous(_) => None,
        }
    }
}
