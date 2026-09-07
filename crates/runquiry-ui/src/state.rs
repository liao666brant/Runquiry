//! 七种数据状态的语义定义（`Ready` + DESIGN.md §6 的六种呈现状态）。
//!
//! 状态只表达「数据为什么不可用」，不表达操作系统判断；平台能力结论由 core
//! 的 `CapabilityStatus` 传入，UI 只按本模块的语义呈现。

use gpui_component::IconName;

/// 数据区域可能处于的状态。
///
/// `Ready` 表示有可用数据；其余六种是 DESIGN.md 规定的统一呈现状态。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DataState {
    /// 数据已加载且非空（gallery 的默认展示态）。
    #[default]
    Ready,
    /// 采集进行中：保留上下文，禁止误导性交互。
    Loading,
    /// 采集成功但结果为空集合。
    Empty,
    /// 采集或分析失败：说明发生了什么与下一步。
    Error,
    /// 平台不支持该能力：是能力边界，不是错误。
    Unsupported,
    /// 平台支持但当前环境不可用（采集器缺失、服务不可达）：是环境边界，
    /// 不是平台缺陷，也不是可重试失败的错误。
    Unavailable,
    /// 权限不足：是权限边界，不是错误。
    PermissionDenied,
}

impl DataState {
    /// CLI/QA 使用的稳定键名（小写 kebab-case）。
    pub const ALL: [Self; 7] = [
        Self::Ready,
        Self::Loading,
        Self::Empty,
        Self::Error,
        Self::Unsupported,
        Self::Unavailable,
        Self::PermissionDenied,
    ];

    /// 返回 CLI/QA 使用的稳定键名。
    pub const fn key(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Loading => "loading",
            Self::Empty => "empty",
            Self::Error => "error",
            Self::Unsupported => "unsupported",
            Self::Unavailable => "unavailable",
            Self::PermissionDenied => "permission-denied",
        }
    }

    /// 从 CLI/QA 键名解析状态；`ready` 表示正常态。
    pub fn parse(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|state| state.key() == key)
    }

    /// 状态图标；`None` 表示应使用不定进度指示（loading）。
    pub const fn icon(self) -> Option<IconName> {
        match self {
            Self::Ready | Self::Loading => None,
            Self::Empty => Some(IconName::Inbox),
            Self::Error => Some(IconName::CircleX),
            Self::Unavailable => Some(IconName::Info),
            // 能力与权限边界都不用「错误」图标：语义不同，不能只靠颜色区分。
            Self::Unsupported => Some(IconName::Dash),
            Self::PermissionDenied => Some(IconName::EyeOff),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_round_trip() {
        let mut keys: Vec<&str> = DataState::ALL.iter().map(|state| state.key()).collect();
        keys.sort_unstable();
        let unique = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), unique, "状态键名存在重复");

        for state in DataState::ALL {
            assert_eq!(DataState::parse(state.key()), Some(state));
        }
    }

    #[test]
    fn parse_is_case_sensitive_and_rejects_unknown() {
        assert_eq!(DataState::parse("loading"), Some(DataState::Loading));
        assert_eq!(
            DataState::parse("permission-denied"),
            Some(DataState::PermissionDenied)
        );
        assert_eq!(DataState::parse("Loading"), None);
        assert_eq!(DataState::parse(""), None);
        assert_eq!(DataState::parse("failed"), None);
    }

    /// 状态呈现组件用到的状态不得只靠颜色区分：每种都要有可辨识的图标语义。
    ///
    /// `Ready` 有数据，不使用状态呈现组件，因此不参与图标对比。
    /// `IconName` 没有实现 `PartialEq`/`Debug`，这里比较图标的资源路径。
    #[test]
    fn state_views_have_distinct_icons() {
        let icon_path = |state: DataState| state.icon().map(gpui_component::IconNamed::path);

        let icons: Vec<_> = DataState::ALL
            .iter()
            .filter(|state| **state != DataState::Ready)
            .map(|state| (state.key(), icon_path(*state)))
            .collect();

        for (ix, (_, icon)) in icons.iter().enumerate() {
            assert!(
                !icons[ix + 1..].iter().any(|(_, other)| *other == *icon),
                "存在两种状态共用同一图标：{icons:?}"
            );
        }
        assert!(
            icon_path(DataState::Loading).is_none(),
            "loading 应使用不定进度指示"
        );
    }
}
