//! Runquiry Component Gallery（A4 组件实验台）。
//!
//! 独立开发入口：`cargo run -p runquiry-app --example gallery -- [参数]`。
//! 不进产品窗口、不进安装包（examples 不参与发布）。数据全部合成，不读取本机
//! 进程、端口、文件、容器或环境变量。
//!
//! 设计规范见仓库根 `DESIGN.md`；设计系统实现见 `runquiry-ui`。
//!
//! 模块划分：[`cli`]（参数与用法）、[`table`]（数据表）、[`overlays`]（覆盖层）、
//! [`data`]（合成数据）；本文件保留 gallery 状态与窗口装配。

mod cli;
mod data;
mod overlays;
mod table;

use gpui::AnyElement;
use gpui::{
    App, AppContext as _, Bounds, Context, Entity, FocusHandle, Focusable as _,
    InteractiveElement as _, IntoElement, KeyBinding, ParentElement as _, Render, Styled as _,
    Window, WindowBounds, WindowKind, WindowOptions, div, px, size,
};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, Root, Selectable as _, Sizable as _, ThemeMode,
    button::{Button, ButtonVariants as _},
    h_flex,
    sidebar::{Sidebar, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
    status_bar::StatusBar,
    table::{DataTable, TableState},
    tree::{TreeEntry, TreeState, tree},
    v_flex,
};

use gpui_component_assets::Assets;
use runquiry_ui::{DataState, Dict, Lang, StateView};

use crate::cli::{Args, MIN_SIZE, parse_args, print_cli_error};
use crate::overlays::{Overlay, open_alert_overlay, open_sheet_overlay, push_notice};
use crate::table::GalleryTable;

/// 窗口标题（QA 用 `xdotool` 定位）。
const WINDOW_TITLE: &str = "Runquiry Gallery";
/// 侧栏键盘路径的 key context。
const SIDEBAR_CONTEXT: &str = "GallerySidebar";
/// 焦点状态行显示的稳定区域 ID。
const FOCUS_SIDEBAR: &str = "sidebar-panel";
const FOCUS_TOOLBAR: &str = "toolbar";
const FOCUS_TABLE: &str = "data-table";
const FOCUS_CONTENT: &str = "content-panel";
/// 树每层缩进：结构深度的几何表达（物理像素，见 `list_item` 注释）。
const TREE_INDENT: f32 = 16.;

/// 侧栏键盘路径动作（`actions!` 宏生成的类型无法逐个补文档，在此统一豁免）。
///
/// 只有方向键：侧栏项的「激活」语义在 gallery 里没有可执行的状态变化
/// （方向键已经把选择落到目标上，点击与 `NavNext`/`NavPrev` 更新的是同一个
/// `nav` 字段），绑定 Enter 只能得到一个空操作，反而让键盘路径看起来比实际
/// 更完整。B4 的真实侧栏若需要 Enter 确认（例如切换工作区），由应用层在该
/// 语义存在时再绑定。
#[allow(missing_docs, clippy::derive_partial_eq_without_eq)]
mod nav_actions {
    gpui::actions!(gallery, [NavNext, NavPrev]);
}
use nav_actions::{NavNext, NavPrev};

/// 侧栏导航目标。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Nav {
    Overview,
    Processes,
    Ports,
    Containers,
    FileLocks,
}

/// 侧栏导航顺序，即视觉顺序（方向键路径与之一致）。
const NAV: [Nav; 5] = [
    Nav::Overview,
    Nav::Processes,
    Nav::Ports,
    Nav::Containers,
    Nav::FileLocks,
];

impl Nav {
    /// 导航图标。
    const fn icon(self) -> IconName {
        match self {
            Self::Overview => IconName::LayoutDashboard,
            Self::Processes => IconName::Cpu,
            Self::Ports => IconName::Network,
            Self::Containers => IconName::SquareTerminal,
            Self::FileLocks => IconName::HardDrive,
        }
    }

    /// 导航文案。
    const fn label(self, dict: &Dict) -> &'static str {
        match self {
            Self::Overview => dict.nav_overview,
            Self::Processes => dict.nav_processes,
            Self::Ports => dict.nav_ports,
            Self::Containers => dict.nav_containers,
            Self::FileLocks => dict.nav_file_locks,
        }
    }

    /// 在 [`NAV`] 中的位置。
    fn index(self) -> usize {
        NAV.iter().position(|item| *item == self).unwrap_or(0)
    }

    /// 按位置取导航项，越界回退到第一项。
    fn at(ix: usize) -> Self {
        NAV.get(ix).copied().unwrap_or(Self::Overview)
    }
}

/// 工具栏控件的稳定身份：元素 ID 与 tab 顺序号成对定义。
///
/// 构建按钮（[`control_button`]）与解读焦点读数（[`Gallery::describe_focus`]）
/// 用同一份定义，避免两处漂移。控件从 1 起按工具栏的视觉顺序编号——编号同时
/// 决定 Tab 遍历顺序，因此必须与视觉顺序一致；区域容器的编号（见
/// [`TAB_SIDEBAR`] 等）必须排在全部控件之后，否则 Tab 会先跳进侧栏/表格。
#[derive(Clone, Copy)]
struct Control {
    /// 稳定元素 ID，也是状态行显示的控件名。
    id: &'static str,
    /// tab 顺序号。
    tab: isize,
}

impl Control {
    const fn new(id: &'static str, tab: isize) -> Self {
        Self { id, tab }
    }
}

const THEME_LIGHT: Control = Control::new("theme-light", 1);
const THEME_DARK: Control = Control::new("theme-dark", 2);
const LANG_EN: Control = Control::new("lang-en", 3);
const LANG_ZH_CN: Control = Control::new("lang-zh-cn", 4);
const STATE_READY: Control = Control::new("ready", 5);
const STATE_LOADING: Control = Control::new("loading", 6);
const STATE_EMPTY: Control = Control::new("empty", 7);
const STATE_ERROR: Control = Control::new("error", 8);
const STATE_UNSUPPORTED: Control = Control::new("unsupported", 9);
const STATE_PERMISSION_DENIED: Control = Control::new("permission-denied", 10);
const OPEN_NOTIFICATION: Control = Control::new("open-notification", 11);
const OPEN_ALERT: Control = Control::new("open-alert", 12);
const OPEN_SHEET: Control = Control::new("open-sheet", 13);
/// Sheet 的 Close 按钮：不在工具栏，但走同一套焦点读数。
const SHEET_CLOSE: Control = Control::new("sheet-close", 14);

/// 区域容器的 tab 顺序号：排在全部控件（1..=14）之后，让 Tab 从工具栏出发
/// 先走完工具栏按钮，再进入侧栏与数据区。工具栏容器本身保持 0（初始焦点）。
const TAB_SIDEBAR: isize = 15;
const TAB_CONTENT: isize = 16;
const TAB_DATA_TABLE: isize = 17;

/// 全部已命名控件，供焦点读数按 tab 顺序号反查控件 ID。
const CONTROLS: [Control; 14] = [
    THEME_LIGHT,
    THEME_DARK,
    LANG_EN,
    LANG_ZH_CN,
    STATE_READY,
    STATE_LOADING,
    STATE_EMPTY,
    STATE_ERROR,
    STATE_UNSUPPORTED,
    STATE_PERMISSION_DENIED,
    OPEN_NOTIFICATION,
    OPEN_ALERT,
    OPEN_SHEET,
    SHEET_CLOSE,
];

/// gallery 主视图。
struct Gallery {
    mode: ThemeMode,
    lang: Lang,
    state: DataState,
    nav: Nav,
    table: Entity<TableState<GalleryTable>>,
    tree: Entity<TreeState>,
    sidebar_focus: FocusHandle,
    toolbar_focus: FocusHandle,
    content_focus: FocusHandle,
    /// 表格的 FocusHandle（组件持有，这里留一份用于解读焦点读数与设置 tab 序）。
    table_focus: FocusHandle,
}

impl Gallery {
    /// 创建 gallery 视图。
    fn new(args: &Args, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let table = cx.new(|cx| {
            TableState::new(GalleryTable::new(args.lang, args.state), window, cx)
                .row_selectable(true)
                .sortable(true)
                .col_resizable(true)
        });
        // 表格的 FocusHandle 由组件持有；tab 顺序号在共享句柄上设置一次即可
        // （`FocusHandle::tab_index` 写回底层 FocusRef），让表格排在工具栏按钮
        // 之后，见 [`TAB_DATA_TABLE`]。
        let table_focus = table.read(cx).focus_handle(cx).tab_index(TAB_DATA_TABLE);
        let items: Vec<_> = data::tree_items();
        let tree = cx.new(|cx| TreeState::new(cx).items(items));

        Self {
            mode: args.mode,
            lang: args.lang,
            state: args.state,
            nav: Nav::Overview,
            table,
            tree,
            sidebar_focus: cx.focus_handle().tab_index(TAB_SIDEBAR),
            toolbar_focus: cx.focus_handle(),
            content_focus: cx.focus_handle().tab_index(TAB_CONTENT),
            table_focus,
        }
    }

    /// 当前语言与状态，供启动覆盖层读取。
    const fn overlay_context(&self) -> (Lang, DataState) {
        (self.lang, self.state)
    }

    /// 切换主题：主题切换不改变工作区状态。
    fn set_mode(&mut self, mode: ThemeMode, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.mode == mode {
            return;
        }
        runquiry_ui::theme::apply(mode, Some(window), cx);
        self.mode = mode;
        cx.notify();
    }

    /// 切换语言：字典与表头同步刷新，不丢失状态与选择。
    fn set_lang(&mut self, lang: Lang, cx: &mut Context<'_, Self>) {
        if self.lang == lang {
            return;
        }
        self.lang = lang;
        let state = self.state;
        self.table.update(cx, |table, cx| {
            table.delegate_mut().lang = lang;
            let columns = GalleryTable::new(lang, state).columns;
            table.delegate_mut().columns = columns;
            table.refresh(cx);
        });
        cx.notify();
    }

    /// 切换数据状态。
    fn set_state(&mut self, state: DataState, cx: &mut Context<'_, Self>) {
        if self.state == state {
            return;
        }
        self.state = state;
        self.table.update(cx, |table, cx| {
            table.delegate_mut().state = state;
            table.refresh(cx);
        });
        cx.notify();
    }

    /// 侧栏键盘路径：上/下移动选择（选择即激活，见 `nav_actions` 注释）。
    fn move_nav(&mut self, step: isize, cx: &mut Context<'_, Self>) {
        let total = NAV.len().cast_signed();
        let next_ix = (self.nav.index().cast_signed() + step).rem_euclid(total);
        let next = Nav::at(next_ix.cast_unsigned());
        self.nav = next;
        cx.notify();
    }

    /// 当前焦点对应的稳定读数。
    ///
    /// 侧栏 / 工具栏 / 表格 / 内容面板是 gallery 自己的 `FocusHandle`，读作
    /// 区域 ID；库内控件（Button 等）不暴露 `FocusHandle`，gallery 以显式 tab
    /// 顺序号为它们命名（见 [`CONTROLS`]），焦点行因此能区分具体控件；仍未
    /// 命名的焦点（树等库内区域）以 `unnamed region#tab-N` 兜底。
    fn describe_focus(&self, window: &Window, cx: &App) -> String {
        let dict = Dict::of(self.lang);
        let Some(focused) = window.focused(cx) else {
            return dict.focus_none.to_string();
        };
        if focused == self.sidebar_focus {
            FOCUS_SIDEBAR.to_string()
        } else if focused == self.toolbar_focus {
            FOCUS_TOOLBAR.to_string()
        } else if focused == self.table_focus {
            FOCUS_TABLE.to_string()
        } else if focused == self.content_focus {
            FOCUS_CONTENT.to_string()
        } else if let Some(control) = CONTROLS.iter().find(|c| c.tab == focused.tab_index) {
            control.id.to_string()
        } else {
            format!("{}#tab-{}", dict.focus_unnamed, focused.tab_index)
        }
    }

    fn render_toolbar(&self, dict: &Dict, cx: &Context<'_, Self>) -> impl IntoElement {
        h_flex()
            .id(FOCUS_TOOLBAR)
            .track_focus(&self.toolbar_focus)
            .flex_wrap()
            .items_center()
            .gap_4()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().title_bar)
            .child(self.theme_group(dict, cx))
            .child(self.lang_group(dict, cx))
            .child(self.state_group(dict, cx))
            .child(Self::overlay_group(dict, cx))
    }

    /// 主题切换组。
    fn theme_group(&self, dict: &Dict, cx: &Context<'_, Self>) -> impl IntoElement {
        let mode = self.mode;
        let light = control_button(THEME_LIGHT, dict.theme_light, mode == ThemeMode::Light)
            .on_click(cx.listener(|this, _, window, cx| {
                this.set_mode(ThemeMode::Light, window, cx);
            }));
        let dark = control_button(THEME_DARK, dict.theme_dark, mode == ThemeMode::Dark).on_click(
            cx.listener(|this, _, window, cx| {
                this.set_mode(ThemeMode::Dark, window, cx);
            }),
        );

        group(dict.theme_group, vec![light.into(), dark.into()], cx)
    }

    /// 语言切换组：切换不丢失当前状态与选择。
    fn lang_group(&self, dict: &Dict, cx: &Context<'_, Self>) -> impl IntoElement {
        let en = control_button(LANG_EN, dict.lang_en, self.lang == Lang::En).on_click(
            cx.listener(|this, _, _, cx| {
                this.set_lang(Lang::En, cx);
            }),
        );
        let zh = control_button(LANG_ZH_CN, dict.lang_zh_cn, self.lang == Lang::ZhCn).on_click(
            cx.listener(|this, _, _, cx| {
                this.set_lang(Lang::ZhCn, cx);
            }),
        );

        group(dict.lang_group, vec![en.into(), zh.into()], cx)
    }

    /// 五种状态 + 正常态的切换组。
    fn state_group(&self, dict: &Dict, cx: &Context<'_, Self>) -> impl IntoElement {
        let state = self.state;
        let items: Vec<AnyElement> = [
            (DataState::Ready, STATE_READY, dict.state_ready),
            (DataState::Loading, STATE_LOADING, dict.state_loading),
            (DataState::Empty, STATE_EMPTY, dict.state_empty),
            (DataState::Error, STATE_ERROR, dict.state_error),
            (
                DataState::Unsupported,
                STATE_UNSUPPORTED,
                dict.state_unsupported,
            ),
            (
                DataState::PermissionDenied,
                STATE_PERMISSION_DENIED,
                dict.state_permission_denied,
            ),
        ]
        .into_iter()
        .map(|(target, control, label)| {
            control_button(control, label, state == target).on_click(cx.listener(
                move |this, _, _, cx| {
                    this.set_state(target, cx);
                },
            ))
        })
        .map(Into::into)
        .collect();

        group(dict.state_group, items, cx)
    }

    /// 覆盖层触发按钮组。
    fn overlay_group(dict: &Dict, cx: &Context<'_, Self>) -> impl IntoElement {
        let sheet = control_button(OPEN_SHEET, dict.open_sheet, false).on_click(cx.listener(
            |this, _, window, cx| {
                open_sheet_overlay(this.overlay_context(), window, cx);
            },
        ));
        let alert = control_button(OPEN_ALERT, dict.open_alert, false).on_click(cx.listener(
            |this, _, window, cx| {
                open_alert_overlay(this.overlay_context(), window, cx);
            },
        ));
        let notice = control_button(OPEN_NOTIFICATION, dict.open_notification, false).on_click(
            cx.listener(|this, _, window, cx| {
                push_notice(this.overlay_context(), window, cx);
            }),
        );

        group(
            dict.overlay_group,
            vec![notice.into(), alert.into(), sheet.into()],
            cx,
        )
    }

    fn render_sidebar(&self, dict: &Dict, cx: &Context<'_, Self>) -> impl IntoElement {
        let nav = self.nav;

        div()
            .id(FOCUS_SIDEBAR)
            .track_focus(&self.sidebar_focus)
            .key_context(SIDEBAR_CONTEXT)
            .on_action(cx.listener(|this, _: &NavNext, _, cx| this.move_nav(1, cx)))
            .on_action(cx.listener(|this, _: &NavPrev, _, cx| this.move_nav(-1, cx)))
            // 没有绑定 Enter：选择已经由方向键落定，见 `nav_actions` 注释。
            .h_full()
            .child(
                Sidebar::new("gallery-sidebar")
                    .w_56()
                    .header(SidebarHeader::new().child(dict.gallery_title))
                    .child(
                        SidebarGroup::new(dict.nav_group).child(SidebarMenu::new().children(
                            NAV.map(|item| {
                                SidebarMenuItem::new(item.label(dict))
                                    .icon(item.icon())
                                    .active(nav == item)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.nav = item;
                                        cx.notify();
                                    }))
                            }),
                        )),
                    ),
            )
    }

    fn render_content(&self, dict: &Dict, cx: &Context<'_, Self>) -> impl IntoElement {
        let state = self.state;
        let (title, description) = self.lang.state_copy(state);

        v_flex()
            .id(FOCUS_CONTENT)
            .track_focus(&self.content_focus)
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full()
            .gap_3()
            .p_3()
            .bg(cx.theme().background)
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .gap_2()
                    .child(section_title(dict.table_title, cx))
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .child(DataTable::new(&self.table).stripe(true).bordered(true)),
                    ),
            )
            .child(
                v_flex()
                    .h_56()
                    .gap_2()
                    .child(section_title(dict.tree_title, cx))
                    .child(
                        div()
                            .h_full()
                            .min_h_0()
                            .child(if state == DataState::Ready {
                                tree(&self.tree, |_ix, entry, _selected, _window, cx| {
                                    list_item(entry, cx)
                                })
                                .into_any_element()
                            } else {
                                StateView::new(state, title)
                                    .description(description)
                                    .note(dict.interaction_note)
                                    .into_any_element()
                            }),
                    ),
            )
    }

    fn render_status_bar(
        &self,
        dict: &Dict,
        focus: &str,
        _cx: &mut Context<'_, Self>,
    ) -> impl IntoElement {
        StatusBar::new()
            .left(format!("{}: {}", dict.focus_prefix, focus))
            .child(format!(
                "{}: {}",
                dict.state_prefix,
                dict.state_name(self.state)
            ))
            .right(format!(
                "{} · {}",
                if self.mode.is_dark() {
                    dict.theme_dark
                } else {
                    dict.theme_light
                },
                self.lang.code()
            ))
            .into_any_element()
    }
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let dict = Dict::of(self.lang);
        let focus = self.describe_focus(window, cx);

        // 覆盖层（Sheet/Dialog/Notification）由内容视图负责渲染：当前
        // gpui-component 的 Root 只协调它们的状态与焦点恢复，不主动绘制。
        // 顺序即层级：Sheet → Dialog → Notification（最上层最后绘制）。
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        v_flex()
            .size_full()
            .relative()
            .text_color(cx.theme().foreground)
            .bg(cx.theme().background)
            .child(self.render_toolbar(&dict, cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.render_sidebar(&dict, cx))
                    .child(self.render_content(&dict, cx)),
            )
            .child(self.render_status_bar(&dict, &focus, cx))
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
    }
}

/// 一组带说明标签的紧凑控件；条目经 `Vec<AnyElement>` 传递，避免大体积控件数组上栈。
fn group(label: &'static str, items: Vec<AnyElement>, cx: &App) -> impl IntoElement {
    h_flex()
        .flex_wrap()
        .min_w_0()
        .items_center()
        .gap_1()
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label),
        )
        .children(items)
}

/// 控件按钮：ID 与 tab 顺序号来自 [`Control`]，选中态由 `.selected()` 表达。
fn control_button(control: Control, label: &'static str, selected: bool) -> Button {
    Button::new(control.id)
        .small()
        .ghost()
        .label(label)
        .selected(selected)
        .tab_index(control.tab)
}

/// 数据区小节标题。
fn section_title(label: &'static str, cx: &App) -> impl IntoElement {
    div()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(label)
}

/// 树节点行：图标 + 文案。
///
/// 层级缩进是几何（每层一格），不属于间距 token：它表达的是结构深度，
/// 与相邻内容的间距语义无关。元素 ID 用树节点的稳定领域 ID
/// （`entry.item().id`），与表格行的领域 ID 做法一致：展开/收起或增删节点时
/// 身份不随下标漂移。
fn list_item(entry: &TreeEntry, cx: &App) -> gpui_component::list::ListItem {
    let icon = if !entry.is_folder() {
        IconName::File
    } else if entry.is_expanded() {
        IconName::FolderOpen
    } else {
        IconName::Folder
    };
    // depth 很小（树层级），usize→f32 精度损失可忽略；缩进是几何而非数据。
    #[allow(clippy::cast_precision_loss)]
    let indent = px(TREE_INDENT * entry.depth() as f32);

    gpui_component::list::ListItem::new(entry.item().id.clone())
        .w_full()
        .rounded(cx.theme().radius)
        .pl(indent)
        .pr(px(TREE_INDENT))
        .child(
            h_flex()
                .gap_2()
                .child(Icon::new(icon).text_color(cx.theme().muted_foreground))
                .child(entry.item().label.clone()),
        )
}

fn main() {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            print_cli_error(&message);
            std::process::exit(2);
        }
    };

    let app = gpui_platform::application().with_assets(Assets);
    app.run(move |cx| {
        gpui_component::init(cx);
        runquiry_ui::theme::install(cx);
        runquiry_ui::theme::apply(args.mode, None, cx);
        cx.bind_keys([
            KeyBinding::new("down", NavNext, Some(SIDEBAR_CONTEXT)),
            KeyBinding::new("up", NavPrev, Some(SIDEBAR_CONTEXT)),
        ]);
        cx.activate(true);

        let bounds = Bounds::centered(None, size(px(args.width), px(args.height)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(MIN_SIZE.0), px(MIN_SIZE.1))),
            kind: WindowKind::Normal,
            ..Default::default()
        };

        // Root 必须是窗口第一级视图（gpui-component Root 契约）。
        let mut gallery_entity = None;
        match cx.open_window(options, |window, cx| {
            window.set_window_title(WINDOW_TITLE);
            let gallery = cx.new(|cx| Gallery::new(&args, window, cx));
            gallery_entity = Some(gallery.clone());
            cx.new(|cx| Root::new(gallery, window, cx))
        }) {
            Ok(window) => {
                let _ = window.update(cx, |_, window, cx| {
                    window.activate_window();
                    // 初始焦点落在工具栏：Tab 顺序从这里开始（视觉顺序的首个区域），
                    // 也让底部状态行在启动后立刻有可观察的焦点证据。
                    if let Some(gallery) = gallery_entity.as_ref() {
                        let focus = gallery.read(cx).toolbar_focus.clone();
                        window.focus(&focus, cx);
                    }
                    // Root 已就位，启动参数要求的覆盖层此时才能打开；覆盖层内部会
                    // 再次更新 Root，因此推迟到本次 update 结束后执行，避免重入。
                    if let Some(overlay) = args.open {
                        window.defer(cx, move |window, cx| match overlay {
                            Overlay::Sheet => {
                                open_sheet_overlay((args.lang, args.state), window, cx);
                            }
                            Overlay::Alert => {
                                open_alert_overlay((args.lang, args.state), window, cx);
                            }
                            Overlay::Notification => {
                                push_notice((args.lang, args.state), window, cx);
                            }
                        });
                    }
                });
            }
            Err(err) => {
                print_cli_error(&format!("打开窗口失败: {err}"));
                cx.quit();
            }
        }
    });
}
