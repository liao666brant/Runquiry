//! gallery 与设计系统组件用的最小双语字典（A4 阶段，不引入 rust-i18n）。
//!
//! 文案以常量字段表达：两种语言各有一份完整 [`Dict`]，字段集合由编译器保证
//! 一致，测试再保证内容非空且确实区分语言。B4 引入 rust-i18n 时，这里的键名
//! 将逐条迁移为翻译键。设计系统组件（如 [`crate::state_view::StateView`]）自身
//! 不持有文案，文本一律由调用方传入。

/// gallery 与设计系统文案使用的语言。
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

    /// BCP 47 语言标签，也是 CLI 取值。
    pub const fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::ZhCn => "zh-CN",
        }
    }

    /// 从 CLI 取值解析语言。
    pub fn parse(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|lang| lang.code() == code)
    }

    /// 返回该语言下某种状态的文案（标题，说明）。
    ///
    /// `Ready` 表示有数据，不使用状态呈现组件，因此返回空文案。
    pub const fn state_copy(self, state: crate::state::DataState) -> (&'static str, &'static str) {
        let dict = Dict::of(self);
        match state {
            crate::state::DataState::Ready => ("", ""),
            crate::state::DataState::Loading => (dict.loading_title, dict.loading_description),
            crate::state::DataState::Empty => (dict.empty_title, dict.empty_description),
            crate::state::DataState::Error => (dict.error_title, dict.error_description),
            crate::state::DataState::Unsupported => {
                (dict.unsupported_title, dict.unsupported_description)
            }
            crate::state::DataState::PermissionDenied => (
                dict.permission_denied_title,
                dict.permission_denied_description,
            ),
        }
    }
}

/// 一种语言的全部界面文案。
#[derive(Clone, Copy, Debug)]
pub struct Dict {
    /// 窗口内容标题。
    pub gallery_title: &'static str,
    /// 主题分组标签。
    pub theme_group: &'static str,
    /// 浅色主题按钮。
    pub theme_light: &'static str,
    /// 深色主题按钮。
    pub theme_dark: &'static str,
    /// 语言分组标签。
    pub lang_group: &'static str,
    /// 英文按钮。
    pub lang_en: &'static str,
    /// 简体中文按钮。
    pub lang_zh_cn: &'static str,
    /// 状态分组标签。
    pub state_group: &'static str,
    /// 正常态按钮。
    pub state_ready: &'static str,
    /// 加载态按钮。
    pub state_loading: &'static str,
    /// 空态按钮。
    pub state_empty: &'static str,
    /// 错误态按钮。
    pub state_error: &'static str,
    /// 不支持态按钮。
    pub state_unsupported: &'static str,
    /// 权限不足态按钮。
    pub state_permission_denied: &'static str,
    /// 覆盖层分组标签。
    pub overlay_group: &'static str,
    /// 打开 Sheet 按钮。
    pub open_sheet: &'static str,
    /// 打开 `AlertDialog` 按钮。
    pub open_alert: &'static str,
    /// 发送 Notification 按钮。
    pub open_notification: &'static str,
    /// 侧栏导航分组标签。
    pub nav_group: &'static str,
    /// 导航项：概览。
    pub nav_overview: &'static str,
    /// 导航项：进程。
    pub nav_processes: &'static str,
    /// 导航项：端口。
    pub nav_ports: &'static str,
    /// 导航项：容器。
    pub nav_containers: &'static str,
    /// 导航项：文件锁。
    pub nav_file_locks: &'static str,
    /// 数据表区域标题。
    pub table_title: &'static str,
    /// 树区域标题。
    pub tree_title: &'static str,
    /// Sheet 标题。
    pub sheet_title: &'static str,
    /// Sheet 关闭按钮。
    pub close: &'static str,
    /// 列名：进程名。
    pub column_name: &'static str,
    /// 列名：可执行路径。
    pub column_path: &'static str,
    /// 列名：PID。
    pub column_pid: &'static str,
    /// 列名：端口。
    pub column_port: &'static str,
    /// 加载态标题。
    pub loading_title: &'static str,
    /// 加载态说明。
    pub loading_description: &'static str,
    /// 空态标题。
    pub empty_title: &'static str,
    /// 空态说明。
    pub empty_description: &'static str,
    /// 错误态标题。
    pub error_title: &'static str,
    /// 错误态说明。
    pub error_description: &'static str,
    /// 不支持态标题。
    pub unsupported_title: &'static str,
    /// 不支持态说明。
    pub unsupported_description: &'static str,
    /// 权限不足态标题。
    pub permission_denied_title: &'static str,
    /// 权限不足态说明。
    pub permission_denied_description: &'static str,
    /// 重试按钮。
    pub retry: &'static str,
    /// 确认对话框标题。
    pub alert_title: &'static str,
    /// 确认对话框说明。
    pub alert_description: &'static str,
    /// 确认按钮（破坏性结果）。
    pub alert_confirm: &'static str,
    /// 状态行「焦点」前缀。
    pub focus_prefix: &'static str,
    /// 焦点为空时的占位文案。
    pub focus_none: &'static str,
    /// 焦点落在未命名区域时的占位文案。
    pub focus_unnamed: &'static str,
    /// 状态行「数据状态」前缀。
    pub state_prefix: &'static str,
    /// 无焦点可操作时的提示（loading/unsupported 等）。
    pub interaction_note: &'static str,
    /// Ready 态通知的正文（`state_copy` 对 Ready 返回空文案，通知需要一句正文）。
    pub notice_ready_body: &'static str,
}

impl Dict {
    /// 返回指定语言的字典。
    pub const fn of(lang: Lang) -> Self {
        match lang {
            Lang::En => EN,
            Lang::ZhCn => ZH_CN,
        }
    }

    /// 返回状态在工具栏/状态栏中使用的短名称。
    pub const fn state_name(&self, state: crate::state::DataState) -> &'static str {
        match state {
            crate::state::DataState::Ready => self.state_ready,
            crate::state::DataState::Loading => self.state_loading,
            crate::state::DataState::Empty => self.state_empty,
            crate::state::DataState::Error => self.state_error,
            crate::state::DataState::Unsupported => self.state_unsupported,
            crate::state::DataState::PermissionDenied => self.state_permission_denied,
        }
    }

    /// 全部条目，用于测试字典完整性。
    pub fn entries(self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("gallery_title", self.gallery_title),
            ("theme_group", self.theme_group),
            ("theme_light", self.theme_light),
            ("theme_dark", self.theme_dark),
            ("lang_group", self.lang_group),
            ("lang_en", self.lang_en),
            ("lang_zh_cn", self.lang_zh_cn),
            ("state_group", self.state_group),
            ("state_ready", self.state_ready),
            ("state_loading", self.state_loading),
            ("state_empty", self.state_empty),
            ("state_error", self.state_error),
            ("state_unsupported", self.state_unsupported),
            ("state_permission_denied", self.state_permission_denied),
            ("overlay_group", self.overlay_group),
            ("open_sheet", self.open_sheet),
            ("open_alert", self.open_alert),
            ("open_notification", self.open_notification),
            ("nav_group", self.nav_group),
            ("nav_overview", self.nav_overview),
            ("nav_processes", self.nav_processes),
            ("nav_ports", self.nav_ports),
            ("nav_containers", self.nav_containers),
            ("nav_file_locks", self.nav_file_locks),
            ("table_title", self.table_title),
            ("tree_title", self.tree_title),
            ("sheet_title", self.sheet_title),
            ("close", self.close),
            ("column_name", self.column_name),
            ("column_path", self.column_path),
            ("column_pid", self.column_pid),
            ("column_port", self.column_port),
            ("loading_title", self.loading_title),
            ("loading_description", self.loading_description),
            ("empty_title", self.empty_title),
            ("empty_description", self.empty_description),
            ("error_title", self.error_title),
            ("error_description", self.error_description),
            ("unsupported_title", self.unsupported_title),
            ("unsupported_description", self.unsupported_description),
            ("permission_denied_title", self.permission_denied_title),
            (
                "permission_denied_description",
                self.permission_denied_description,
            ),
            ("retry", self.retry),
            ("alert_title", self.alert_title),
            ("alert_description", self.alert_description),
            ("alert_confirm", self.alert_confirm),
            ("focus_prefix", self.focus_prefix),
            ("focus_none", self.focus_none),
            ("focus_unnamed", self.focus_unnamed),
            ("state_prefix", self.state_prefix),
            ("interaction_note", self.interaction_note),
            ("notice_ready_body", self.notice_ready_body),
        ]
    }
}

/// 英文文案（默认与兜底语言）。
const EN: Dict = Dict {
    gallery_title: "Component gallery",
    theme_group: "Theme",
    theme_light: "Light",
    theme_dark: "Dark",
    lang_group: "Language",
    lang_en: "English",
    lang_zh_cn: "中文",
    state_group: "Data state",
    state_ready: "Ready",
    state_loading: "Loading",
    state_empty: "Empty",
    state_error: "Error",
    state_unsupported: "Unsupported",
    state_permission_denied: "Permission denied",
    overlay_group: "Overlays",
    open_sheet: "Sheet…",
    open_alert: "Alert dialog…",
    open_notification: "Notification",
    nav_group: "Workspaces",
    nav_overview: "Overview",
    nav_processes: "Processes",
    nav_ports: "Ports",
    nav_containers: "Containers",
    nav_file_locks: "File Locks",
    table_title: "Processes",
    tree_title: "Sources",
    sheet_title: "Process detail",
    close: "Close",
    column_name: "Name",
    column_path: "Executable path",
    column_pid: "PID",
    column_port: "Port",
    loading_title: "Collecting process data",
    loading_description: "The first sample is being read. Values appear once it completes.",
    empty_title: "No processes match",
    empty_description: "Nothing matches the current query. Adjust the filters and try again.",
    error_title: "Couldn't read the process table",
    error_description: "The inventory source returned an error. Retry to collect again.",
    unsupported_title: "Not available on this platform",
    unsupported_description: "This workspace has no collector for the current platform.",
    permission_denied_title: "Permission denied",
    permission_denied_description: "Run Runquiry with access to the process inventory to continue.",
    retry: "Retry",
    alert_title: "Terminate “containerd-shim”?",
    alert_description: "The process will stop immediately. Child processes keep running.",
    alert_confirm: "Terminate",
    focus_prefix: "Focus",
    focus_none: "none",
    focus_unnamed: "unnamed region",
    state_prefix: "Data state",
    interaction_note: "Actions are unavailable while data is loading or unsupported.",
    notice_ready_body: "Synthetic sample loaded: 8 processes across 4 sources.",
};

/// 简体中文文案。
const ZH_CN: Dict = Dict {
    gallery_title: "组件实验台",
    theme_group: "主题",
    theme_light: "浅色",
    theme_dark: "深色",
    lang_group: "语言",
    lang_en: "English",
    lang_zh_cn: "中文",
    state_group: "数据状态",
    state_ready: "正常",
    state_loading: "加载中",
    state_empty: "空",
    state_error: "错误",
    state_unsupported: "不支持",
    state_permission_denied: "权限不足",
    overlay_group: "覆盖层",
    open_sheet: "侧板…",
    open_alert: "确认对话框…",
    open_notification: "通知",
    nav_group: "工作区",
    nav_overview: "概览",
    nav_processes: "进程",
    nav_ports: "端口",
    nav_containers: "容器",
    nav_file_locks: "文件锁",
    table_title: "进程",
    tree_title: "来源",
    sheet_title: "进程详情",
    close: "关闭",
    column_name: "名称",
    column_path: "可执行路径",
    column_pid: "PID",
    column_port: "端口",
    loading_title: "正在采集进程数据",
    loading_description: "第一次采样读取中，采样完成后显示数值。",
    empty_title: "没有匹配的进程",
    empty_description: "当前筛选没有结果，请调整条件后重试。",
    error_title: "无法读取进程列表",
    error_description: "采集源返回错误，可重试以重新采集。",
    unsupported_title: "当前平台不支持",
    unsupported_description: "此工作区在当前平台没有可用采集器。",
    permission_denied_title: "权限不足",
    permission_denied_description: "需要进程清单读取权限才能继续。",
    retry: "重试",
    alert_title: "终止 “containerd-shim”？",
    alert_description: "该进程会立即停止，子进程不受影响。",
    alert_confirm: "终止",
    focus_prefix: "焦点",
    focus_none: "无",
    focus_unnamed: "未命名区域",
    state_prefix: "数据状态",
    interaction_note: "数据加载中或平台不支持时，操作不可用。",
    notice_ready_body: "已加载合成示例：8 个进程、4 个来源。",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lang_codes_round_trip() {
        for lang in Lang::ALL {
            assert_eq!(Lang::parse(lang.code()), Some(lang));
        }
        assert_eq!(Lang::parse("zh"), None);
        assert_eq!(Lang::parse("zh-CN"), Some(Lang::ZhCn));
        assert_eq!(Lang::parse(""), None);
    }

    /// 两种语言的键集合必须一致，且每条文案非空。
    #[test]
    fn both_languages_are_complete() {
        let en = Lang::En.code();
        let zh = Lang::ZhCn.code();
        let en_entries = Dict::of(Lang::En).entries();
        let zh_entries = Dict::of(Lang::ZhCn).entries();

        assert_eq!(en_entries.len(), zh_entries.len());
        for ((en_key, en_value), (zh_key, zh_value)) in en_entries.iter().zip(zh_entries.iter()) {
            assert_eq!(en_key, zh_key, "{en} 与 {zh} 键顺序不一致");
            assert!(!en_value.trim().is_empty(), "{en_key} 的 {en} 文案为空");
            assert!(!zh_value.trim().is_empty(), "{en_key} 的 {zh} 文案为空");
        }
    }

    /// 字典必须真的区分语言，避免把同一份文案误标成两套。
    #[test]
    fn languages_differ() {
        let en_entries = Dict::of(Lang::En).entries();
        let zh_entries = Dict::of(Lang::ZhCn).entries();
        let differing = en_entries
            .iter()
            .zip(zh_entries.iter())
            .filter(|(en, zh)| en.1 != zh.1)
            .count();
        assert!(differing > 20, "两种语言文案几乎相同（{differing} 条不同）");
    }

    /// 每种状态都必须有标题与说明文案。
    #[test]
    fn every_state_has_copy() {
        for state in crate::state::DataState::ALL {
            for lang in Lang::ALL {
                let (title, description) = lang.state_copy(state);
                if state == crate::DataState::Ready {
                    assert_eq!(title, "");
                    assert_eq!(description, "");
                } else {
                    assert!(
                        !title.trim().is_empty(),
                        "{} 缺少 {state:?} 标题",
                        lang.code()
                    );
                    assert!(
                        !description.trim().is_empty(),
                        "{} 缺少 {state:?} 说明",
                        lang.code()
                    );
                }
            }
        }
    }
}
