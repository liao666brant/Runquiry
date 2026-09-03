//! Runquiry 设计系统主题（DESIGN.md 的唯一实现处）。
//!
//! 原始色值（hex）只允许出现在本模块的 [`PALETTE`] 表中；所有组件调用点一律
//! 通过 `cx.theme()` 语义 token 读取。本模块把 Runquiry 的中性石墨表面与钴蓝
//! 交互强调色投影到 gpui-component 的主题系统，浅色/深色两套配置共用同一张
//! token 表（同一 token 名在两种模式下都必须出现，见下方测试）。

use std::rc::Rc;

use gpui::{App, Window};
use gpui_component::{Theme, ThemeConfig, ThemeConfigColors, ThemeMode};

/// 浅色主题在主题注册表中的名称。
pub const LIGHT_THEME_NAME: &str = "Runquiry Light";
/// 深色主题在主题注册表中的名称。
pub const DARK_THEME_NAME: &str = "Runquiry Dark";

/// 语义 token 表：`(token 名, 浅色值, 深色值)`。
///
/// 只列出 Runquiry 需要覆盖的语义角色；未列出的角色沿用 gpui-component
/// 内置浅/深主题默认值（语义绿/黄/红即保持库默认，Runquiry 不重新发明）。
/// 所有值必须是 `#RRGGBB` 或 `#RRGGBBAA`。
const PALETTE: &[(&str, &str, &str)] = &[
    // ---- 表面（中性石墨/灰色） ----
    ("background", "#FAFAFB", "#16171A"),
    ("foreground", "#18181B", "#E5E6E9"),
    ("border", "#E3E4E8", "#2C2F35"),
    ("muted.background", "#F1F2F4", "#22242A"),
    ("muted.foreground", "#6E7178", "#9BA0A8"),
    ("secondary.background", "#E9EAEE", "#26282E"),
    ("secondary.foreground", "#27272A", "#E5E6E9"),
    ("popover.background", "#FFFFFF", "#1C1E22"),
    ("popover.foreground", "#18181B", "#E5E6E9"),
    ("accent.background", "#EDEEF1", "#26282E"),
    ("accent.foreground", "#27272A", "#E5E6E9"),
    ("input.border", "#D8DAE0", "#34383F"),
    // ---- 交互强调（钴蓝）：只用于交互与选中，不表达成功/警告/危险 ----
    ("primary.background", "#1D4ED8", "#3B82F6"),
    ("primary.hover.background", "#1E40AF", "#60A5FA"),
    ("primary.active.background", "#1E3A8A", "#2563EB"),
    ("primary.foreground", "#FFFFFF", "#0B1220"),
    ("ring", "#1D4ED8", "#3B82F6"),
    ("selection.background", "#1D4ED85C", "#3B82F666"),
    ("drag.border", "#1D4ED8", "#3B82F6"),
    ("drop_target.background", "#1D4ED826", "#3B82F63D"),
    ("list.active.background", "#1D4ED81F", "#3B82F633"),
    ("list.active.border", "#1D4ED8", "#3B82F6"),
    ("table.active.background", "#1D4ED81F", "#3B82F633"),
    ("table.active.border", "#1D4ED8", "#3B82F6"),
    // ---- 侧栏与固定栏（石墨） ----
    ("sidebar.background", "#F2F3F5", "#1B1D21"),
    ("sidebar.border", "#E3E4E8", "#2C2F35"),
    ("sidebar.foreground", "#27272A", "#E5E6E9"),
    ("sidebar.accent.background", "#E4E6EA", "#26282E"),
    ("sidebar.accent.foreground", "#1B1D21", "#E5E6E9"),
    ("sidebar.primary.background", "#1D4ED8", "#3B82F6"),
    ("sidebar.primary.foreground", "#FFFFFF", "#0B1220"),
    ("title_bar.background", "#F7F8F9", "#1B1D21"),
    ("title_bar.border", "#E3E4E8", "#2C2F35"),
    ("status_bar.background", "#F2F3F5", "#1B1D21"),
    ("status_bar.border", "#E3E4E8", "#2C2F35"),
    // ---- 数据区（石墨灰，紧凑表格读数） ----
    ("table.background", "#FFFFFF", "#1C1E22"),
    ("table.even.background", "#F6F7F8", "#1B1D21"),
    ("table.head.background", "#F1F2F4", "#202227"),
    ("table.head.foreground", "#52565E", "#A6ABB4"),
    ("table.hover.background", "#EDEEF1", "#23252B"),
    ("table.row.border", "#E3E4E8B3", "#2C2F35B3"),
    ("list.background", "#FFFFFF", "#1C1E22"),
    ("list.even.background", "#F6F7F8", "#1B1D21"),
    ("list.head.background", "#F1F2F4", "#202227"),
    ("list.hover.background", "#EDEEF1", "#23252B"),
];

/// 把单个 token 写入 [`ThemeConfigColors`]，返回是否命中已知 token 名。
fn set_token(colors: &mut ThemeConfigColors, token: &str, value: &str) -> bool {
    let field = match token {
        "background" => &mut colors.background,
        "foreground" => &mut colors.foreground,
        "border" => &mut colors.border,
        "muted.background" => &mut colors.muted,
        "muted.foreground" => &mut colors.muted_foreground,
        "secondary.background" => &mut colors.secondary,
        "secondary.foreground" => &mut colors.secondary_foreground,
        "popover.background" => &mut colors.popover,
        "popover.foreground" => &mut colors.popover_foreground,
        "accent.background" => &mut colors.accent,
        "accent.foreground" => &mut colors.accent_foreground,
        "input.border" => &mut colors.input,
        "primary.background" => &mut colors.primary,
        "primary.hover.background" => &mut colors.primary_hover,
        "primary.active.background" => &mut colors.primary_active,
        "primary.foreground" => &mut colors.primary_foreground,
        "ring" => &mut colors.ring,
        "selection.background" => &mut colors.selection,
        "drag.border" => &mut colors.drag_border,
        "drop_target.background" => &mut colors.drop_target,
        "list.active.background" => &mut colors.list_active,
        "list.active.border" => &mut colors.list_active_border,
        "table.active.background" => &mut colors.table_active,
        "table.active.border" => &mut colors.table_active_border,
        "sidebar.background" => &mut colors.sidebar,
        "sidebar.border" => &mut colors.sidebar_border,
        "sidebar.foreground" => &mut colors.sidebar_foreground,
        "sidebar.accent.background" => &mut colors.sidebar_accent,
        "sidebar.accent.foreground" => &mut colors.sidebar_accent_foreground,
        "sidebar.primary.background" => &mut colors.sidebar_primary,
        "sidebar.primary.foreground" => &mut colors.sidebar_primary_foreground,
        "title_bar.background" => &mut colors.title_bar,
        "title_bar.border" => &mut colors.title_bar_border,
        "status_bar.background" => &mut colors.status_bar,
        "status_bar.border" => &mut colors.status_bar_border,
        "table.background" => &mut colors.table,
        "table.even.background" => &mut colors.table_even,
        "table.head.background" => &mut colors.table_head,
        "table.head.foreground" => &mut colors.table_head_foreground,
        "table.hover.background" => &mut colors.table_hover,
        "table.row.border" => &mut colors.table_row_border,
        "list.background" => &mut colors.list,
        "list.even.background" => &mut colors.list_even,
        "list.head.background" => &mut colors.list_head,
        "list.hover.background" => &mut colors.list_hover,
        _ => return false,
    };
    *field = Some(value.into());
    true
}

/// 生成指定模式的 Runquiry 主题配置。
fn build_config(mode: ThemeMode) -> (ThemeConfig, Vec<&'static str>) {
    let is_dark = mode.is_dark();
    let mut colors = ThemeConfigColors::default();
    let mut applied = Vec::with_capacity(PALETTE.len());

    for (token, light, dark) in PALETTE {
        let value = if is_dark { dark } else { light };
        if set_token(&mut colors, token, value) {
            applied.push(*token);
        }
    }

    let config = ThemeConfig {
        name: if is_dark {
            DARK_THEME_NAME
        } else {
            LIGHT_THEME_NAME
        }
        .into(),
        mode,
        // 紧凑圆角：诊断控制台的密度读数（DESIGN.md「圆角」节）。
        radius: Some(4),
        radius_lg: Some(6),
        colors,
        ..Default::default()
    };
    (config, applied)
}

/// 安装 Runquiry 浅/深主题（幂等，必须在 `gpui_component::init` 之后调用）。
///
/// 只替换主题配置来源，不改变当前激活模式；激活模式由 [`apply`] 控制。
pub fn install(cx: &mut App) {
    if !cx.has_global::<Theme>() {
        Theme::change(ThemeMode::Light, None, cx);
    }

    let light = Rc::new(build_config(ThemeMode::Light).0);
    let dark = Rc::new(build_config(ThemeMode::Dark).0);
    let theme = Theme::global_mut(cx);
    theme.light_theme = light;
    theme.dark_theme = dark;
}

/// 切换到指定模式，并把当前主题投影到 Base 层（滚动条、窗口边框）。
pub fn apply(mode: ThemeMode, window: Option<&mut Window>, cx: &mut App) {
    install(cx);
    Theme::change(mode, window, cx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::Rgba;

    /// 每个原始色值都必须能被 gpui 解析为有效颜色。
    #[test]
    fn palette_values_are_valid_rgba() {
        for (token, light, dark) in PALETTE {
            for (mode, value) in [("light", *light), ("dark", *dark)] {
                assert!(
                    Rgba::try_from(value).is_ok(),
                    "token {token} 的 {mode} 色值 {value} 不是合法的 #RRGGBB(AA)"
                );
            }
        }
    }

    /// 两种模式必须覆盖完全相同的 token 集合，避免主题切换后出现缺口。
    #[test]
    fn light_and_dark_cover_the_same_tokens() {
        let (light_config, light_tokens) = build_config(ThemeMode::Light);
        let (dark_config, dark_tokens) = build_config(ThemeMode::Dark);

        assert_eq!(light_tokens, dark_tokens, "浅色与深色 token 集合不一致");
        assert!(light_tokens.len() >= 40, "覆盖的 token 数量过少");
        // 每个 PALETTE 条目都必须命中 set_token 的映射：token 名拼错会被静默
        // 跳过（回退库默认值），在此显式失败而不是留下难察觉的主题缺口。
        assert_eq!(
            light_tokens.len(),
            PALETTE.len(),
            "PALETTE 中存在未命中任何主题字段的 token 名"
        );

        // 同一 token 在两种模式下都必须给出不同的具体值（否则该 token 不该出现在表里）。
        for (token, light, dark) in PALETTE {
            assert_ne!(light, dark, "token {token} 在浅色与深色模式下取值相同");
        }

        // 主题名与模式必须一致，供主题注册表与状态栏读取。
        assert_eq!(light_config.name, LIGHT_THEME_NAME);
        assert_eq!(dark_config.name, DARK_THEME_NAME);
        assert!(!light_config.mode.is_dark());
        assert!(dark_config.mode.is_dark());
    }

    /// 钴蓝强调色必须与石墨表面区分：primary 亮度显著不同于 background。
    #[test]
    fn accent_is_not_a_surface_value() {
        let (light, _) = build_config(ThemeMode::Light);
        let (dark, _) = build_config(ThemeMode::Dark);

        assert_ne!(light.colors.primary, light.colors.background);
        assert_ne!(light.colors.primary, light.colors.sidebar);
        assert_ne!(dark.colors.primary, dark.colors.background);
        assert_ne!(dark.colors.primary, dark.colors.sidebar);
    }
}
