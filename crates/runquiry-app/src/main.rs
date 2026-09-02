//! Runquiry 桌面应用入口。
//!
//! A1 阶段仅包含验证技术栈所需的最小窗口：初始化 gpui-component，
//! 并保证 [`gpui_component::Root`] 是窗口的第一级视图。
//! 产品工作区由后续任务实现。

use gpui::{
    AppContext as _, Bounds, Context, IntoElement, ParentElement as _, Render, Styled as _, Window,
    WindowBounds, WindowKind, WindowOptions, div, px, size,
};
use gpui_component::{ActiveTheme as _, Root};
use gpui_component_assets::Assets;

/// 最小应用壳层视图，仅用于验证 GPUI 与 gpui-component 技术栈。
struct ShellView;

impl Render for ShellView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child("Runquiry")
    }
}

fn main() {
    let app = gpui_platform::application().with_assets(Assets);

    app.run(|cx| {
        gpui_component::init(cx);
        cx.activate(true);

        let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(960.), px(640.))),
            kind: WindowKind::Normal,
            ..Default::default()
        };

        // Root 必须是窗口第一级视图（gpui-component Root 契约）。
        match cx.open_window(options, |window, cx| {
            let shell = cx.new(|_| ShellView);
            cx.new(|cx| Root::new(shell, window, cx))
        }) {
            Ok(window) => {
                let _ = window.update(cx, |_, window, _| {
                    window.activate_window();
                    window.set_window_title("Runquiry");
                });
            }
            Err(err) => {
                eprintln!("打开主窗口失败: {err}");
                cx.quit();
            }
        }
    });
}
