//! shell 后续绑定到真实 GPUI action 的纯事件契约。

/// Processes 页面的键盘命令。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessCommand {
    /// 刷新当前工作区。
    Refresh,
    /// 聚焦调查输入。
    FocusQuery,
    /// 切换工作区（1..=4）。
    Workspace(u8),
}

impl ProcessCommand {
    /// 产品装配层消费的唯一全局快捷键契约。
    pub const fn bindings() -> [(&'static str, Self); 12] {
        [
            ("ctrl-r", Self::Refresh),
            ("ctrl-k", Self::FocusQuery),
            ("ctrl-1", Self::Workspace(1)),
            ("ctrl-2", Self::Workspace(2)),
            ("ctrl-3", Self::Workspace(3)),
            ("ctrl-4", Self::Workspace(4)),
            ("cmd-r", Self::Refresh),
            ("cmd-k", Self::FocusQuery),
            ("cmd-1", Self::Workspace(1)),
            ("cmd-2", Self::Workspace(2)),
            ("cmd-3", Self::Workspace(3)),
            ("cmd-4", Self::Workspace(4)),
        ]
    }
}
