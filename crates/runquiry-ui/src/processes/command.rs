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
    /// 打开当前详情的动作菜单。
    OpenActionMenu,
    /// 在已打开的动作菜单中选择动作。
    SelectAction(ProcessActionShortcut),
    /// 关闭动作菜单且不执行。
    CloseActionMenu,
}

/// witr 动作菜单的单键选择。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessActionShortcut {
    /// SIGKILL。
    Kill,
    /// SIGTERM。
    Terminate,
    /// SIGSTOP。
    Pause,
    /// SIGCONT。
    Resume,
    /// 使用当前输入值调整 nice。
    Renice,
}

impl ProcessCommand {
    /// 产品装配层消费的唯一全局快捷键契约。
    pub const fn bindings() -> [(&'static str, Self); 21] {
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
            ("a", Self::OpenActionMenu),
            ("shift-a", Self::OpenActionMenu),
            ("k", Self::SelectAction(ProcessActionShortcut::Kill)),
            ("t", Self::SelectAction(ProcessActionShortcut::Terminate)),
            ("p", Self::SelectAction(ProcessActionShortcut::Pause)),
            ("r", Self::SelectAction(ProcessActionShortcut::Resume)),
            ("n", Self::SelectAction(ProcessActionShortcut::Renice)),
            ("escape", Self::CloseActionMenu),
            ("q", Self::CloseActionMenu),
        ]
    }
}
