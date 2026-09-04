//! 目标解析的匹配纯函数、多结果容器与输入边界解析（B1）。
//!
//! 语义来自 [witr 行为契约](../docs/witr-parity.md) §2：
//! * `exact` 的 cmdline 匹配按完整 token / 路径段匹配（witr
//!   `internal/target/resolve.go` 的 `matchesExactToken`）；
//! * `exact=false` 为大小写不敏感的子串匹配；
//! * 多结果不自动选择，进入候选表（`Resolution::Ambiguous` 携带完整候选）；
//! * 字符串→目标值的边界解析只在入口发生一次（PID 正整数、端口 1-65535、
//!   查询串非空），内部只传类型化值。

use std::path::PathBuf;

use crate::model::error::InspectError;
use crate::model::ids::{Pid, Port};

/// 完整 token 匹配（parity：witr `matchesExactToken`，语义逐字对齐）。
///
/// `query` 为查询串（needle），`token` 为候选命令行文本——单个完整参数或
/// 整条命令行均可（函数内按空白拆分字段，兼容 witr 两种调用形态）。命中
/// 规则：字段与查询串全等，或把字段中的反斜杠归一化为 `/` 后按 `/` 拆
/// 路径段、任一段与查询串全等。
///
/// 大小写语义与 witr 一致：本函数本身大小写敏感；witr 的名称解析由调用方
/// 先把两侧统一转小写再比较（`name_*.go` 均先 `ToLower`），exact 匹配链
/// 保持该约定。
pub fn matches_exact_token(query: &str, token: &str) -> bool {
    for part in token.split_whitespace() {
        if part == query {
            return true;
        }
        // 归一化反斜杠为斜杠，统一按路径段拆分（Windows 风格路径同样生效）。
        let normalized = part.replace('\\', "/");
        if normalized.split('/').any(|segment| segment == query) {
            return true;
        }
    }
    false
}

/// 大小写不敏感的子串匹配（parity：`exact=false` 的名称匹配）。
pub fn matches_fuzzy(query: &str, candidate: &str) -> bool {
    candidate.to_lowercase().contains(&query.to_lowercase())
}

/// 目标解析结果：唯一命中或多候选。
///
/// 多结果不自动选择（parity §2/§3：命中多个 PID / 容器时列候选，不自动
/// 选第一项）；[`Resolution::Ambiguous`] 携带完整且稳定排序的候选集合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution<T> {
    /// 唯一命中。
    Unique(T),
    /// 多候选：携带全部命中，顺序由调用方保证稳定（如按 PID 升序）。
    Ambiguous(Vec<T>),
}

impl<T> Resolution<T> {
    /// 命中数量（唯一命中为 1）。
    pub const fn count(&self) -> usize {
        match self {
            Self::Unique(_) => 1,
            Self::Ambiguous(candidates) => candidates.len(),
        }
    }

    /// 取全部候选（唯一命中为单元素切片）。
    pub fn candidates(&self) -> &[T] {
        match self {
            Self::Unique(candidate) => std::slice::from_ref(candidate),
            Self::Ambiguous(candidates) => candidates,
        }
    }

    /// 消费自身，返回全部候选（唯一命中为单元素列表）。
    pub fn into_candidates(self) -> Vec<T> {
        match self {
            Self::Unique(candidate) => vec![candidate],
            Self::Ambiguous(candidates) => candidates,
        }
    }
}

/// 目标非法时构造 [`InspectError::InvalidTarget`]（reason 与 witr 错误文案对齐）。
fn invalid_target(reason: impl std::fmt::Display) -> InspectError {
    InspectError::InvalidTarget {
        reason: reason.to_string(),
    }
}

/// 字符串 → PID 边界解析：仅接受正整数（parity：`strconv.Atoi` 成功且
/// `pid > 0`；负数、0、非数字、溢出均为 invalid target）。
///
/// # Errors
/// 输入为空白、非数字、0 或负值时返回 [`InspectError::InvalidTarget`]。
pub fn parse_pid(input: &str) -> Result<Pid, InspectError> {
    let trimmed = input.trim();
    // 与 Go strconv.Atoi 一致接受 "+N" 形式；空串与裸符号拒绝。
    let digits = trimmed.strip_prefix('+').unwrap_or(trimmed);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) || digits.len() > 10 {
        return Err(invalid_target("invalid pid: must be a positive integer"));
    }
    let value: u64 = digits
        .parse()
        .map_err(|_| invalid_target("invalid pid: must be a positive integer"))?;
    let pid_value = u32::try_from(value)
        .map_err(|_| invalid_target("invalid pid: must be a positive integer"))?;
    Pid::new(pid_value).map_err(|_| invalid_target("invalid pid: must be a positive integer"))
}

/// 字符串 → 端口边界解析：仅接受 1-65535（parity：`invalid port: must be
/// between 1 and 65535`）。
///
/// # Errors
/// 输入为空白、非数字或越界时返回 [`InspectError::InvalidTarget`]。
pub fn parse_port(input: &str) -> Result<Port, InspectError> {
    let trimmed = input.trim();
    let value: u64 = trimmed
        .parse()
        .map_err(|_| invalid_target("invalid port: must be between 1 and 65535"))?;
    let port_value = u16::try_from(value)
        .map_err(|_| invalid_target("invalid port: must be between 1 and 65535"))?;
    Port::new(port_value).map_err(|_| invalid_target("invalid port: must be between 1 and 65535"))
}

/// 字符串 → 文件路径边界解析：非空即原样保留（parity：File 目标的路径语义
/// 与符号链接归一化由平台 `FileInventory` 承担，core 不改写输入）。
///
/// # Errors
/// trim 后为空时返回 [`InspectError::InvalidTarget`]。
pub fn parse_file_path(input: &str) -> Result<PathBuf, InspectError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(invalid_target("file path must not be empty"));
    }
    Ok(PathBuf::from(trimmed))
}

/// 名称 / 容器查询串边界解析：trim 后非空（空查询在 witr 的子串语义下会
/// 命中所有进程，Runquiry 在边界显式拒绝）。
///
/// # Errors
/// trim 后为空时返回 [`InspectError::InvalidTarget`]。
pub fn parse_query(input: &str) -> Result<String, InspectError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(invalid_target("query must not be empty"));
    }
    Ok(String::from(trimmed))
}
