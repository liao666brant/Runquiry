//! 语言与 rust-i18n 初始化。
//!
//! 文案一律通过 `rust_i18n::t!` 取自 `locales/`（en 兜底，zh-CN 完整对齐，
//! 完整性由单元测试强制），不再保留任何手写运行时字典。当前 locale 是
//! GPUI 不追踪的全局状态：[`set_language`] 切换后调用方必须显式
//! `cx.notify()` 才会重绘。

use gpui_component::set_locale;

use crate::state::DataState;

/// 界面支持的语言。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Lang {
    /// 英文（默认与兜底）。
    #[default]
    En,
    /// 简体中文。
    ZhCn,
}

impl Lang {
    /// 所有支持的语言。
    pub const ALL: [Self; 2] = [Self::En, Self::ZhCn];

    /// BCP 47 语言标签，也是设置文件与 CLI 的取值。
    pub const fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::ZhCn => "zh-CN",
        }
    }

    /// 从设置文件 / CLI 取值解析语言。
    pub fn parse(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|lang| lang.code() == code)
    }
}

/// 把 gpui-component 的内置文案并入我们的翻译后端（进程内只允许调用一次）。
///
/// 必须在 `gpui_component::init` 之前调用一次；重复调用会被 rust-i18n 断言
/// 拒绝。此后组件文案查找会先经过我们 `locales/` 里的覆盖键。
pub fn extend_component_translations() {
    rust_i18n::extend!(gpui_component);
}

/// 切换界面语言（全局 locale，立即生效于后续 `t!` 查询）。
///
/// 注意：这只改变全局状态，不会触发重绘；调用方需要随后 `cx.notify()`。
pub fn set_language(lang: Lang) {
    set_locale(lang.code());
}

/// 按当前 locale 取文案。
///
/// `rust_i18n::t!` 只能在声明 `i18n!` 的 crate 内展开（宏体引用本 crate 的
/// 后端），gallery 等外部使用方一律通过本函数取翻译：键未命中时按
/// `fallback = "en"` 回退，仍无则回显键名（调用方应在测试中覆盖全部键）。
pub fn tr(key: &str) -> String {
    crate::_rust_i18n_translate(&rust_i18n::locale(), key).into_owned()
}

/// 数据状态的（标题，说明）文案；[`DataState::Ready`] 不使用状态呈现组件，
/// 返回空串对。
///
/// 供 gallery 实验台使用（`gallery.<state>_title` /
/// `gallery.<state>_description`）；产品工作区一律使用
/// [`workspace_state_copy`]，避免实验台文案进入产品界面。状态短名见
/// [`state_name`]。
pub fn state_copy(state: DataState) -> (String, String) {
    match state {
        DataState::Ready => (String::new(), String::new()),
        DataState::Loading => (
            tr("gallery.loading_title"),
            tr("gallery.loading_description"),
        ),
        DataState::Empty => (tr("gallery.empty_title"), tr("gallery.empty_description")),
        DataState::Error => (tr("gallery.error_title"), tr("gallery.error_description")),
        DataState::Unsupported => (
            tr("gallery.unsupported_title"),
            tr("gallery.unsupported_description"),
        ),
        DataState::Unavailable => (
            tr("gallery.unavailable_title"),
            tr("gallery.unavailable_description"),
        ),
        DataState::PermissionDenied => (
            tr("gallery.permission_denied_title"),
            tr("gallery.permission_denied_description"),
        ),
    }
}

/// 产品工作区数据状态的（标题，说明）文案（`workspace_state.<state>.{title,
/// description}`）；[`DataState::Ready`] 不使用状态呈现组件，返回空串对。
pub fn workspace_state_copy(state: DataState) -> (String, String) {
    let suffix = match state {
        DataState::Ready => return (String::new(), String::new()),
        DataState::Loading => "loading",
        DataState::Empty => "empty",
        DataState::Error => "error",
        DataState::Unsupported => "unsupported",
        DataState::Unavailable => "unavailable",
        DataState::PermissionDenied => "permission_denied",
    };
    (
        tr(&format!("workspace_state.{suffix}.title")),
        tr(&format!("workspace_state.{suffix}.description")),
    )
}

/// 状态在工具栏/状态栏中使用的短名称。
pub fn state_name(state: DataState) -> String {
    let key = match state {
        DataState::Ready => "gallery.state_ready",
        DataState::Loading => "gallery.state_loading",
        DataState::Empty => "gallery.state_empty",
        DataState::Error => "gallery.state_error",
        DataState::Unsupported => "gallery.state_unsupported",
        DataState::Unavailable => "gallery.state_unavailable",
        DataState::PermissionDenied => "gallery.state_permission_denied",
    };
    tr(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// 两种语言的键集合必须一致，且每条文案非空。
    ///
    /// 直接读翻译后端：fallback（en）会掩盖 zh-CN 缺键，所以必须按键集合
    /// 对比，不能只靠 `t!` 不报错。
    #[test]
    fn both_languages_have_the_same_keys() {
        let backend = crate::_rust_i18n_backend();
        let collect = |locale: &str| -> BTreeMap<String, String> {
            backend
                .messages_for_locale(locale)
                .unwrap_or_default()
                .into_iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect()
        };

        let en = collect("en");
        let zh_cn = collect("zh-CN");
        assert_eq!(
            rust_i18n::t!("app.name"),
            "Runquiry",
            "t! 必须命中翻译而不是回显键名"
        );
        assert!(!en.is_empty(), "en 翻译为空：locales/en.yml 未被编译进后端");
        assert_eq!(en.len(), zh_cn.len(), "两种语言的键数量不一致");
        for (key, en_value) in &en {
            // `unwrap_or_else(panic!)` 被本仓库 lint 禁止，用 contains_key 表达缺键。
            assert!(zh_cn.contains_key(key), "键 {key} 缺少 zh-CN 文案");
            let zh_value = zh_cn.get(key).map(String::as_str).unwrap_or_default();
            assert!(!en_value.trim().is_empty(), "键 {key} 的 en 文案为空");
            assert!(!zh_value.trim().is_empty(), "键 {key} 的 zh-CN 文案为空");
        }
    }

    /// 键集合必须真的区分语言：绝大多数键的中英文文案不同。
    #[test]
    fn languages_differ() {
        let backend = crate::_rust_i18n_backend();
        let count = |locale: &str| -> usize {
            backend
                .messages_for_locale(locale)
                .unwrap_or_default()
                .iter()
                .filter(|(key, value)| {
                    let en = backend.translate("en", key).unwrap_or_default();
                    en.as_ref() != value.as_ref()
                })
                .count()
        };
        assert!(
            count("zh-CN") > 0,
            "zh-CN 与 en 文案完全相同，疑似复制了同一份"
        );
    }

    /// `Lang` 与 locale 标签互转，且 `set_language` 真实切换全局 locale。
    #[test]
    fn lang_round_trips_and_switches_locale() {
        for lang in Lang::ALL {
            assert_eq!(Lang::parse(lang.code()), Some(lang));
            set_language(lang);
            assert_eq!(&*gpui_component::locale(), lang.code());
        }
        assert_eq!(Lang::parse("zh"), None);
        set_language(Lang::En);
    }
}
