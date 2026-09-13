//! B8 的无副作用合成 100k 行桌面 QA 入口。

pub(crate) mod backend;

use std::sync::Arc;

use gpui_kit::assets::Assets;
use gpui_kit::component::{Root, ThemeMode};
use gpui_kit::{
    AppContext as _, Bounds, Entity, WindowBounds, WindowKind, WindowOptions, px, size,
};
use runquiry_ui::backend::WorkspaceBackend;
use runquiry_ui::{AppShell, Lang, ShellStartup, WorkspaceId, set_language};

use crate::backend::SyntheticBackend;

const WINDOW_TITLE: &str = "Runquiry Scale QA";

fn main() {
    let backend: Arc<dyn WorkspaceBackend> = match SyntheticBackend::new() {
        Ok(backend) => Arc::new(backend),
        Err(_) => return,
    };
    let app = gpui_kit::application().with_assets(Assets);
    app.run(move |cx| {
        runquiry_ui::locale::extend_component_translations();
        gpui_kit::init(cx);
        runquiry_ui::theme::install(cx);
        runquiry_ui::theme::apply(ThemeMode::Dark, None, cx);
        set_language(Lang::En);
        cx.activate(true);

        let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(960.), px(640.))),
            kind: WindowKind::Normal,
            ..Default::default()
        };
        let startup = ShellStartup {
            theme: ThemeMode::Dark,
            language: Lang::En,
            workspace: WorkspaceId::Processes,
            hidden_columns: Default::default(),
        };
        let mut shell_entity: Option<Entity<AppShell>> = None;
        let opened = cx.open_window(options, |window, cx| {
            window.set_window_title(WINDOW_TITLE);
            let shell = cx.new(|cx| AppShell::new(startup, Arc::clone(&backend), window, cx));
            shell_entity = Some(shell.clone());
            cx.new(|cx| Root::new(shell, window, cx))
        });
        match opened {
            Ok(window) => {
                let _ = window.update(cx, |_, window, cx| {
                    window.activate_window();
                    if let Some(shell) = shell_entity {
                        shell.update(cx, AppShell::refresh_active);
                    }
                });
            }
            Err(_) => cx.quit(),
        }
    });
}
