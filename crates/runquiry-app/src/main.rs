//! Runquiry 桌面应用入口与装配层。
//!
//! 职责：把设计系统与产品壳层（runquiry-ui）装配进真实窗口——初始化
//! gpui-component（Root 为窗口第一级视图）、应用启动设置、绑定快捷键，
//! 并把壳层的设置变化事件持久化到 allowlist 白名单内的设置文件
//! （见 [`settings`]）。平台端口由 [`backend`] 在应用边界装配，UI 不直接
//! 访问操作系统或容器 CLI。

pub mod backend;
mod settings;

use std::sync::Arc;

use gpui::{
    App, AppContext as _, Bounds, Entity, Focusable as _, Global, KeyBinding, Window, WindowBounds,
    WindowKind, WindowOptions, px, size,
};
use gpui_component::{Root, ThemeMode};
use gpui_component_assets::Assets;

use backend::{PlatformBackend, UnavailableBackend};
use runquiry_ui::backend::WorkspaceBackend;
use runquiry_ui::processes::{ProcessActionShortcut, ProcessCommand};
use runquiry_ui::shell::{
    ShellEvent, ShellStartup,
    actions::{
        CloseProcessActions, FocusQuery, KillProcess, OpenProcessActions, PauseProcess,
        RefreshWorkspace, ReniceProcess, ResumeProcess, TerminateProcess, Workspace1, Workspace2,
        Workspace3, Workspace4,
    },
};
use runquiry_ui::{AppShell, set_language};
use settings::{Settings, WindowSize, default_settings_path, load_settings, save_settings};

/// 快捷键：刷新当前工作区（DESIGN.md §8.2 产品键盘路径）。
const KEY_CONTEXT: &str = "RunquiryShell";
/// 窗口标题。
const WINDOW_TITLE: &str = "Runquiry";

/// 进程内全局设置状态：装配层持有的当前设置与文件路径。
#[derive(Debug)]
struct SettingsStore {
    path: Option<std::path::PathBuf>,
    settings: Settings,
}

impl Global for SettingsStore {}

fn main() {
    let settings_path = default_settings_path();
    let settings = load_settings(settings_path.as_deref());

    let app = gpui_platform::application().with_assets(Assets);
    app.run(move |cx| {
        // rust-i18n：先并入 gpui-component 的内置文案，再初始化组件
        // （进程内只允许调用一次，见 runquiry-ui::locale）。
        runquiry_ui::locale::extend_component_translations();
        gpui_component::init(cx);
        runquiry_ui::theme::install(cx);
        cx.bind_keys(
            ProcessCommand::bindings().map(|(key, command)| match command {
                ProcessCommand::Refresh => {
                    KeyBinding::new(key, RefreshWorkspace, Some(KEY_CONTEXT))
                }
                ProcessCommand::FocusQuery => KeyBinding::new(key, FocusQuery, Some(KEY_CONTEXT)),
                ProcessCommand::Workspace(1) => KeyBinding::new(key, Workspace1, Some(KEY_CONTEXT)),
                ProcessCommand::Workspace(2) => KeyBinding::new(key, Workspace2, Some(KEY_CONTEXT)),
                ProcessCommand::Workspace(3) => KeyBinding::new(key, Workspace3, Some(KEY_CONTEXT)),
                ProcessCommand::Workspace(_) => KeyBinding::new(key, Workspace4, Some(KEY_CONTEXT)),
                ProcessCommand::OpenActionMenu => {
                    KeyBinding::new(key, OpenProcessActions, Some(KEY_CONTEXT))
                }
                ProcessCommand::SelectAction(ProcessActionShortcut::Kill) => {
                    KeyBinding::new(key, KillProcess, Some(KEY_CONTEXT))
                }
                ProcessCommand::SelectAction(ProcessActionShortcut::Terminate) => {
                    KeyBinding::new(key, TerminateProcess, Some(KEY_CONTEXT))
                }
                ProcessCommand::SelectAction(ProcessActionShortcut::Pause) => {
                    KeyBinding::new(key, PauseProcess, Some(KEY_CONTEXT))
                }
                ProcessCommand::SelectAction(ProcessActionShortcut::Resume) => {
                    KeyBinding::new(key, ResumeProcess, Some(KEY_CONTEXT))
                }
                ProcessCommand::SelectAction(ProcessActionShortcut::Renice) => {
                    KeyBinding::new(key, ReniceProcess, Some(KEY_CONTEXT))
                }
                ProcessCommand::CloseActionMenu => {
                    KeyBinding::new(key, CloseProcessActions, Some(KEY_CONTEXT))
                }
            }),
        );
        cx.activate(true);

        cx.set_global(SettingsStore {
            path: settings_path,
            settings: settings.clone(),
        });

        // 启动设置：主题、语言（文案全局 locale 必须在首帧渲染前就位）。
        let theme = settings.theme_mode().unwrap_or(ThemeMode::Light);
        runquiry_ui::theme::apply(theme, None, cx);
        set_language(settings.language());

        // 窗口尺寸：设置的尺寸夹取到最小窗口内；无设置用默认值。
        let window_size = settings.window.unwrap_or(WindowSize::DEFAULT).clamped();
        let bounds = Bounds::centered(
            None,
            size(px_dim(window_size.width), px_dim(window_size.height)),
            cx,
        );
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(
                px_dim(WindowSize::MIN.width),
                px_dim(WindowSize::MIN.height),
            )),
            kind: WindowKind::Normal,
            ..Default::default()
        };

        // Root 必须是窗口第一级视图（gpui-component Root 契约）。
        let startup = ShellStartup {
            theme,
            language: settings.language(),
            workspace: settings.last_workspace(),
        };
        let backend: Arc<dyn WorkspaceBackend> = match PlatformBackend::new() {
            Ok(backend) => Arc::new(backend),
            Err(error) => Arc::new(UnavailableBackend::new(error.to_string())),
        };
        let mut shell_entity = None;
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
                    wire_settings(shell_entity.clone(), window, cx);
                    if let Some(shell) = shell_entity {
                        shell.update(cx, AppShell::refresh_active);
                    }
                });
            }
            Err(err) => {
                eprintln!("打开主窗口失败: {err}");
                cx.quit();
            }
        }
    });
}

/// 装配层与壳层的全部接线：订阅设置变化事件落盘、初始焦点、窗口关闭退出。
fn wire_settings(shell: Option<Entity<AppShell>>, window: &mut Window, cx: &mut App) {
    let Some(shell) = shell else {
        return;
    };

    // 设置变化事件：主题/语言/工作区立即落盘。窗口尺寸由平台 bounds 事件
    // 单独维护，因此这里保存的一定是最近一次真实窗口尺寸。
    cx.subscribe(&shell, |_shell, event: &ShellEvent, cx| {
        match *event {
            ShellEvent::ThemeChanged(mode) => {
                cx.global_mut::<SettingsStore>().settings.theme = Some(theme_key(mode).to_string());
            }
            ShellEvent::LanguageChanged(lang) => {
                cx.global_mut::<SettingsStore>().settings.language =
                    Some(String::from(lang.code()));
            }
            ShellEvent::WorkspaceChanged(workspace) => {
                cx.global_mut::<SettingsStore>().settings.last_workspace =
                    Some(String::from(workspace.key()));
            }
        }
        persist_settings(cx);
    })
    .detach();

    // 锁定 GPUI 的 X11 后端可能收不到 WM 关闭回调，因此平台 bounds 事件
    // 同时更新内存并落盘，保证仅调整窗口尺寸的会话也能恢复最后尺寸。
    shell.update(cx, |_shell, cx| {
        cx.observe_window_bounds(window, |_shell, window, cx| {
            let size = window.bounds().size;
            remember_window_size(
                &mut cx.global_mut::<SettingsStore>().settings,
                u32::from(size.width),
                u32::from(size.height),
            );
            persist_settings(cx);
        })
        .detach();
    });

    // 初始焦点落在工具栏（DESIGN.md §8.2：启动时初始焦点在视觉顺序首个区域）。
    shell.update(cx, |shell, cx| {
        let focus = shell.focus_handle(cx);
        window.focus(&focus, cx);
    });

    // WM 发起关闭时窗口仍可访问：先捕获最终 bounds、持久化，再允许关闭并退出。
    window.on_window_should_close(cx, |window, cx| {
        let size = window.bounds().size;
        remember_window_size(
            &mut cx.global_mut::<SettingsStore>().settings,
            u32::from(size.width),
            u32::from(size.height),
        );
        persist_settings(cx);
        cx.quit();
        true
    });

    // 非 WM 路径移除窗口时仍保证单窗口应用退出。
    cx.on_window_closed(move |cx, _window_id| {
        persist_settings(cx);
        cx.quit();
    })
    .detach();
}

/// 把内存中的设置原子写入设置文件；失败不中断运行（仅提示）。
fn persist_settings(cx: &mut App) {
    let (path, settings) = {
        let store = cx.global_mut::<SettingsStore>();
        (store.path.clone(), store.settings.clone())
    };
    if let Err(err) = save_settings(path.as_deref(), &settings) {
        eprintln!("设置写入失败: {err}");
    }
}

/// 把平台报告的逻辑像素尺寸写入 allowlist 设置。
const fn remember_window_size(settings: &mut Settings, width: u32, height: u32) {
    settings.window = Some(WindowSize { width, height });
}

/// u32 尺寸 → 逻辑像素：窗口尺寸远小于 f32 尾数精度边界，cast 无实际损失。
#[allow(clippy::cast_precision_loss)]
const fn px_dim(value: u32) -> gpui::Pixels {
    px(value as f32)
}

/// 主题的设置文件取值。
const fn theme_key(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::Light => "light",
        ThemeMode::Dark => "dark",
    }
}

#[cfg(test)]
mod tests {
    use super::{Settings, WindowSize, remember_window_size};

    #[test]
    fn window_bounds_event_updates_the_persisted_size() {
        let mut settings = Settings::default();

        remember_window_size(&mut settings, 1_100, 700);

        assert_eq!(
            settings.window,
            Some(WindowSize {
                width: 1_100,
                height: 700,
            })
        );
    }
}
