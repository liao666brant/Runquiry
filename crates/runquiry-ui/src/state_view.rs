//! 统一的数据状态呈现组件。
//!
//! [`StateView`] 是无状态值型组件（`RenderOnce`）：所有文本与动作都由调用方
//! 传入，组件只负责按 [`DataState`] 选择图标、语义色与排布。loading 使用不定
//! 进度指示，其余状态使用语义图标（unsupported 的短横额外带圆形外框，避免与
//! 分隔线混淆），保证状态不只靠颜色区分。

use gpui::{
    AnyElement, App, IntoElement, ParentElement, Pixels, RenderOnce, SharedString, Styled as _,
    Window, div, prelude::FluentBuilder as _, px,
};
use gpui_component::{
    ActiveTheme as _, Icon, Sizable as _, Size, StyledExt as _, h_flex, spinner::Spinner, v_flex,
};

use crate::state::DataState;

/// unsupported 态「不可用」图形的外框直径（物理像素）。
///
/// 与树的层级缩进一样，这是图形的几何尺寸而非间距 token：它表达的是一个
/// 独立的标记符号，不参与内容的节奏。
const UNSUPPORTED_FRAME: Pixels = px(40.);
/// unsupported 态「不可用」图形中短横的长度（物理像素）。
const UNSUPPORTED_GLYPH: Pixels = px(16.);

/// 一个数据区域的统一状态呈现。
///
/// 用于 loading、empty、error、unsupported、unavailable、permission-denied
/// 六种状态；`Ready` 表示有数据，此时不应使用本组件而应直接渲染数据。
#[derive(IntoElement)]
pub struct StateView {
    state: DataState,
    title: SharedString,
    description: Option<SharedString>,
    action: Option<AnyElement>,
    note: Option<SharedString>,
}

impl StateView {
    /// 以状态与标题创建视图；标题必须说明当前对象与发生了什么。
    pub fn new(state: DataState, title: impl Into<SharedString>) -> Self {
        Self {
            state,
            title: title.into(),
            description: None,
            action: None,
            note: None,
        }
    }

    /// 设置补充说明（发生了什么、下一步怎么办）。
    #[must_use]
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// 设置可选动作按钮（例如「重试」）。
    #[must_use]
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }

    /// 设置辅助提示（例如「操作不可用」），以弱化文字呈现在最下方。
    #[must_use]
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// 状态的图标颜色语义：错误用 danger，权限边界用 warning，其余保持中性，
    /// 避免把能力边界画成系统故障。
    fn icon_color(&self, cx: &App) -> gpui::Hsla {
        let theme = cx.theme();
        match self.state {
            DataState::Error => theme.danger,
            // 能力与环境边界保持中性：不是系统故障，不应使用 danger。
            DataState::Unsupported
            | DataState::Unavailable
            | DataState::Ready
            | DataState::Loading
            | DataState::Empty => theme.muted_foreground,
            DataState::PermissionDenied => theme.warning,
        }
    }
}

/// 手写 `Debug`：`AnyElement` 不实现 `Debug`，只输出可读的语义字段。
impl std::fmt::Debug for StateView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateView")
            .field("state", &self.state)
            .field("title", &self.title)
            .field("has_description", &self.description.is_some())
            .field("has_action", &self.action.is_some())
            .field("has_note", &self.note.is_some())
            .finish()
    }
}

impl RenderOnce for StateView {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let icon_color = self.icon_color(cx);
        // loading 用不定进度指示表达「进行中」；其余状态用语义图标。
        let icon: AnyElement = self.state.icon().map_or_else(
            || {
                Spinner::new()
                    .with_size(Size::Medium)
                    .color(icon_color)
                    .into_any_element()
            },
            |name| self.render_icon(name, icon_color),
        );

        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_2()
            .p_4()
            .text_color(cx.theme().muted_foreground)
            .child(icon)
            .child(
                div()
                    .text_sm()
                    .font_medium()
                    .text_color(cx.theme().foreground)
                    .child(self.title),
            )
            .when_some(self.description, |this, description| {
                this.child(div().max_w_96().text_center().text_xs().child(description))
            })
            .when_some(self.action, ParentElement::child)
            .when_some(self.note, |this, note| {
                // 不再叠加 opacity：muted_foreground 本身就是次级层级（约 4.8:1），
                // 再乘 0.8 透明度会跌到 4.5:1 以下，弱化不应以牺牲可读性为代价。
                this.child(div().text_xs().child(note))
            })
    }
}

impl StateView {
    /// 渲染状态的语义图标。
    ///
    /// unsupported 是唯一的例外：裸的短横与分隔线、空态占位几乎无法区分（深色
    /// 下尤其弱）。给它一个带边框的圆形外框，读作「该能力不可用」的标记——仍是
    /// 中性色、不是错误红，但与 error（红 circle-x）、permission-denied（琥珀
    /// eye-off）、empty（inbox）在形状上可区分，满足「状态不只靠颜色」的要求。
    fn render_icon(&self, name: gpui_component::IconName, color: gpui::Hsla) -> AnyElement {
        if self.state != DataState::Unsupported {
            return Icon::new(name)
                .with_size(Size::Large)
                .text_color(color)
                .into_any_element();
        }

        h_flex()
            .size(UNSUPPORTED_FRAME)
            .flex_none()
            .rounded_full()
            .border_1()
            .border_color(color)
            .items_center()
            .justify_center()
            .child(Icon::new(name).size(UNSUPPORTED_GLYPH).text_color(color))
            .into_any_element()
    }
}
