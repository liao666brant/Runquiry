//! 壳层渲染：工具栏、侧栏、响应式主数据/详情区与状态栏。
//!
//! 只做呈现与交互接线；状态与会话语义见 [`super`]。文案经 `t!` 取自
//! `locales/`，主题与语言由 gpui-component 提供。

use rust_i18n::t;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Root, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    sidebar::{Sidebar, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
    status_bar::StatusBar,
    v_flex,
};

use super::actions::RefreshWorkspace;
use super::{AppShell, DETAIL_RATIO, KEY_CONTEXT, MAIN_RATIO};
use crate::locale::Lang;
use crate::session::WorkspaceId;
use crate::state::DataState;
use crate::state_view::StateView;

/// 工具栏控件的稳定元素 ID 与 tab 顺序号（与视觉顺序一致，见 gallery 的做法）。
const TAB_REFRESH: isize = 1;
const TAB_THEME_LIGHT: isize = 2;
const TAB_THEME_DARK: isize = 3;
const TAB_LANG_EN: isize = 4;
const TAB_LANG_ZH_CN: isize = 5;
const INLINE_DETAIL_MIN_WIDTH: u32 = 1_100;

/// 宽窗口内联显示详情；窄窗口为后续按选择打开 Sheet 保留主区宽度。
const fn shows_inline_detail(width: u32) -> bool {
    width >= INLINE_DETAIL_MIN_WIDTH
}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let shows_inline_detail = shows_inline_detail(u32::from(window.bounds().size.width));

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
            .text_color(cx.theme().foreground)
            .bg(cx.theme().background)
            .child(self.render_toolbar(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.render_sidebar(cx))
                    .child(self.render_main_area(shows_inline_detail, cx))
                    .when(shows_inline_detail, |layout| {
                        layout.child(self.render_detail_area(cx))
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
            .on_click(cx.listener(|this, _, _, cx| {
                this.set_language(Lang::En, cx);
            }));
        let zh_cn = Button::new("lang-zh-cn")
            .small()
            .ghost()
            .label(t!("toolbar.language.zh_cn").to_string())
            .tab_index(TAB_LANG_ZH_CN)
            .selected(self.lang == Lang::ZhCn)
            .on_click(cx.listener(|this, _, _, cx| {
                this.set_language(Lang::ZhCn, cx);
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

    /// 主数据区：平台采集端口尚未接入，各工作区展示诚实的能力边界态
    /// （`StateView` 语义，不注入演示数据）。
    fn render_main_area(
        &self,
        shows_inline_detail: bool,
        cx: &Context<'_, Self>,
    ) -> impl IntoElement {
        div()
            .id("main-panel")
            .track_focus(&self.main_focus)
            .flex_grow(MAIN_RATIO)
            .min_w_0()
            .min_h_0()
            .when(shows_inline_detail, |panel| {
                panel.border_r_1().border_color(cx.theme().border)
            })
            .child(
                StateView::new(
                    DataState::Unsupported,
                    t!("main.collector_unavailable.title").to_string(),
                )
                .description(t!("main.collector_unavailable.description").to_string()),
            )
    }

    /// 详情区：占位提示；真实详情由 B5/B6 随选择与平台端口接入。
    fn render_detail_area(&self, _cx: &Context<'_, Self>) -> impl IntoElement {
        div()
            .id("detail-panel")
            .track_focus(&self.detail_focus)
            .flex_grow(DETAIL_RATIO)
            .min_w_0()
            .min_h_0()
            .child(
                StateView::new(DataState::Empty, t!("detail.placeholder.title").to_string())
                    .description(t!("detail.placeholder.description").to_string()),
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
    use super::shows_inline_detail;

    #[test]
    fn hides_inline_detail_when_window_is_1099_pixels_wide() {
        // Given: the widest window in the compact layout range.
        let width = 1_099;

        // When: the master-detail presentation is selected.
        let shows_detail = shows_inline_detail(width);

        // Then: detail does not compress the main workspace.
        assert!(!shows_detail);
    }

    #[test]
    fn shows_inline_detail_when_window_is_1100_pixels_wide() {
        // Given: the first width in the wide layout range.
        let width = 1_100;

        // When: the master-detail presentation is selected.
        let shows_detail = shows_inline_detail(width);

        // Then: detail is rendered alongside the main workspace.
        assert!(shows_detail);
    }
}
