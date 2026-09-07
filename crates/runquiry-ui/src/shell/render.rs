//! 壳层渲染：工具栏、侧栏、响应式主数据/详情区与状态栏。
//!
//! 只做呈现与交互接线；状态与会话语义见 [`super`]。文案经 `t!` 取自
//! `locales/`，主题与语言由 gpui-component 提供。

use rust_i18n::t;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div, px,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Root, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex, h_resizable, resizable_panel,
    sidebar::{Sidebar, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
    status_bar::StatusBar,
    v_flex,
};

use super::actions::{
    FocusQuery, RefreshWorkspace, Workspace1, Workspace2, Workspace3, Workspace4,
};
use super::{AppShell, KEY_CONTEXT};
use crate::locale::Lang;
use crate::session::WorkspaceId;

/// 工具栏控件的稳定元素 ID 与 tab 顺序号（与视觉顺序一致，见 gallery 的做法）。
const TAB_REFRESH: isize = 1;
const TAB_THEME_LIGHT: isize = 2;
const TAB_THEME_DARK: isize = 3;
const TAB_LANG_EN: isize = 4;
const TAB_LANG_ZH_CN: isize = 5;
const MAIN_PANEL_SHARE: f32 = 0.65;

/// 宽窗口内联显示详情；窄窗口为后续按选择打开 Sheet 保留主区宽度。
fn shows_inline_detail(width: gpui::Pixels) -> bool {
    width >= px(1_100.)
}

/// 宽窗口的初始列表宽度。分隔条实际拖拽后的值由 `ResizableState` 保留。
fn initial_main_panel_width(window_width: gpui::Pixels) -> gpui::Pixels {
    let sidebar_width = px(224.);
    let main_minimum = px(360.);
    let detail_minimum = px(280.);
    let available = (window_width - sidebar_width).max(main_minimum + detail_minimum);
    (available * MAIN_PANEL_SHARE).clamp(main_minimum, available - detail_minimum)
}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let shows_inline_detail = shows_inline_detail(window.bounds().size.width);

        // 覆盖层由内容视图负责绘制（DESIGN.md §5），顺序即层级。
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        v_flex()
            .size_full()
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(|this, _: &RefreshWorkspace, _, cx| {
                this.refresh_active(cx);
            }))
            .on_action(cx.listener(|this, _: &FocusQuery, window, cx| this.focus_query(window, cx)))
            .on_action(cx.listener(|this, _: &Workspace1, _, cx| {
                this.switch_workspace(WorkspaceId::Processes, cx);
            }))
            .on_action(cx.listener(|this, _: &Workspace2, _, cx| {
                this.switch_workspace(WorkspaceId::Ports, cx);
            }))
            .on_action(cx.listener(|this, _: &Workspace3, _, cx| {
                this.switch_workspace(WorkspaceId::Containers, cx);
            }))
            .on_action(cx.listener(|this, _: &Workspace4, _, cx| {
                this.switch_workspace(WorkspaceId::FileLocks, cx);
            }))
            .text_color(cx.theme().foreground)
            .bg(cx.theme().background)
            .child(self.render_toolbar(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.render_sidebar(cx))
                    .when(shows_inline_detail, |layout| {
                        layout.child(
                            div()
                                .id("workspace-detail-split-container")
                                .flex_1()
                                .h_full()
                                .min_w_0()
                                .min_h_0()
                                .child(
                                    h_resizable("workspace-detail-split")
                                        .child(
                                            resizable_panel()
                                                .size(initial_main_panel_width(
                                                    window.bounds().size.width,
                                                ))
                                                .size_range(px(360.)..gpui::Pixels::MAX)
                                                .child(self.render_main_area(true, cx)),
                                        )
                                        .child(
                                            resizable_panel()
                                                .size_range(px(280.)..gpui::Pixels::MAX)
                                                .child(self.render_detail_area(cx)),
                                        ),
                                ),
                        )
                    })
                    .when(!shows_inline_detail, |layout| {
                        layout.child(self.render_main_area(false, cx))
                    }),
            )
            .child(self.render_status_bar(cx))
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
    }
}

impl AppShell {
    fn render_toolbar(&self, cx: &Context<'_, Self>) -> impl IntoElement {
        let refreshing = self.session.active_session().is_refreshing();

        h_flex()
            .id("toolbar")
            .track_focus(&self.toolbar_focus)
            .flex_wrap()
            .items_center()
            .gap_4()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().title_bar)
            .child(
                Button::new("toolbar-refresh")
                    .small()
                    .ghost()
                    .label(t!("toolbar.refresh").to_string())
                    .tab_index(TAB_REFRESH)
                    .disabled(refreshing)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.refresh_active(cx);
                    })),
            )
            .child(self.theme_group(cx))
            .child(self.language_group(cx))
    }

    /// 主题切换组：切换不触碰工作区会话。
    fn theme_group(&self, cx: &Context<'_, Self>) -> impl IntoElement {
        let light = Button::new("theme-light")
            .small()
            .ghost()
            .label(t!("toolbar.theme.light").to_string())
            .tab_index(TAB_THEME_LIGHT)
            .selected(self.theme == gpui_component::ThemeMode::Light)
            .on_click(cx.listener(|this, _, window, cx| {
                this.set_theme(gpui_component::ThemeMode::Light, window, cx);
            }));
        let dark = Button::new("theme-dark")
            .small()
            .ghost()
            .label(t!("toolbar.theme.dark").to_string())
            .tab_index(TAB_THEME_DARK)
            .selected(self.theme == gpui_component::ThemeMode::Dark)
            .on_click(cx.listener(|this, _, window, cx| {
                this.set_theme(gpui_component::ThemeMode::Dark, window, cx);
            }));

        h_flex().items_center().gap_1().child(light).child(dark)
    }

    /// 语言切换组：切换后由 `set_language` 显式 notify，界面立即重绘。
    fn language_group(&self, cx: &Context<'_, Self>) -> impl IntoElement {
        let en = Button::new("lang-en")
            .small()
            .ghost()
            .label(t!("toolbar.language.en").to_string())
            .tab_index(TAB_LANG_EN)
            .selected(self.lang == Lang::En)
            .on_click(cx.listener(|this, _, window, cx| {
                this.set_language(Lang::En, window, cx);
            }));
        let zh_cn = Button::new("lang-zh-cn")
            .small()
            .ghost()
            .label(t!("toolbar.language.zh_cn").to_string())
            .tab_index(TAB_LANG_ZH_CN)
            .selected(self.lang == Lang::ZhCn)
            .on_click(cx.listener(|this, _, window, cx| {
                this.set_language(Lang::ZhCn, window, cx);
            }));

        h_flex().items_center().gap_1().child(en).child(zh_cn)
    }

    fn render_sidebar(&self, cx: &Context<'_, Self>) -> impl IntoElement {
        let active = self.active_workspace();

        div()
            .id("sidebar-panel")
            .track_focus(&self.sidebar_focus)
            .h_full()
            .child(
                Sidebar::new("runquiry-sidebar")
                    .w_56()
                    .header(SidebarHeader::new().child(t!("app.name").to_string()))
                    .child(
                        SidebarGroup::new(t!("sidebar.workspaces").to_string()).child(
                            SidebarMenu::new().children(WorkspaceId::ALL.map(|workspace| {
                                SidebarMenuItem::new(workspace_title(workspace))
                                    .active(active == workspace)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.switch_workspace(workspace, cx);
                                    }))
                            })),
                        ),
                    ),
            )
    }

    fn render_status_bar(&self, _cx: &Context<'_, Self>) -> impl IntoElement {
        let interval = self
            .session
            .active_session()
            .refresh_interval()
            .as_secs()
            .to_string();

        StatusBar::new()
            .left(format!(
                "{}: {}",
                t!("status.workspace"),
                workspace_title(self.active_workspace())
            ))
            .child(format!("{}: {}s", t!("status.interval"), interval))
            .right(self.lang.code().to_string())
            .into_any_element()
    }
}

/// 工作区的本地化名称（侧栏与状态栏共用）。
fn workspace_title(workspace: WorkspaceId) -> SharedString {
    let key = match workspace {
        WorkspaceId::Processes => "workspace.processes",
        WorkspaceId::Ports => "workspace.ports",
        WorkspaceId::Containers => "workspace.containers",
        WorkspaceId::FileLocks => "workspace.file_locks",
    };
    t!(key).to_string().into()
}

#[cfg(test)]
mod tests {
    use super::{MAIN_PANEL_SHARE, initial_main_panel_width, shows_inline_detail};
    use gpui::px;

    #[test]
    fn hides_inline_detail_when_window_is_1099_pixels_wide() {
        // Given: the widest window in the compact layout range.
        let width = px(1_099.);

        // When: the master-detail presentation is selected.
        let shows_detail = shows_inline_detail(width);

        // Then: detail does not compress the main workspace.
        assert!(!shows_detail);
    }

    #[test]
    fn shows_inline_detail_when_window_is_1100_pixels_wide() {
        // Given: the first width in the wide layout range.
        let width = px(1_100.);

        // When: the master-detail presentation is selected.
        let shows_detail = shows_inline_detail(width);

        // Then: detail is rendered alongside the main workspace.
        assert!(shows_detail);
    }

    #[test]
    fn wide_layout_starts_with_a_65_35_split_after_the_sidebar() {
        for width in [px(1_100.), px(1_280.)] {
            let available = width - px(224.);
            let main = initial_main_panel_width(width);

            assert!((main.as_f32() / available.as_f32() - MAIN_PANEL_SHARE).abs() < f32::EPSILON);
            assert!(available - main >= px(280.));
        }
    }
}
