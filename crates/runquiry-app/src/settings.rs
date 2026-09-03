//! 设置持久化（allowlist 白名单）。
//!
//! 只允许写入五类设置：主题、语言、窗口尺寸、最后工作区、列布局。
//! 调查输入（目标、PID、进程名、路径、筛选内容）、选择与调查结果
//! **不进设置文件**——它们是会话状态（见 runquiry-ui 的 `AppSession`），
//! 结构上就不存在被序列化的路径。
//!
//! 文件格式为 JSON（serde 已在依赖链）；未知字段一律拒绝
//! （`deny_unknown_fields`）：设置文件版本漂移时宁可回退默认值，也不静默
//! 携带无法解释的字段。加载不存在或损坏时安全回退默认值，不 panic；
//! 写入用「临时文件 + 重命名」原子替换，避免中途崩溃留下半截文件。
//!
//! 路径由调用方注入（[`load_settings`]/[`save_settings`] 的 `path` 参数），
//! 测试用临时目录，不触碰真实用户目录。

// 本模块是 bin crate 的私有模块：条目需要 `pub(crate)` 才能被 crate 根
// （main.rs）访问，而这正是 `redundant_pub_crate` 与 `unreachable_pub`
// 两个 lint 互相冲突的场景（与 gallery example 的做法一致，模块级豁免）。
#![allow(clippy::redundant_pub_crate)]

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use runquiry_ui::Lang;

/// 列布局：工作区键 →（列 ID → 列宽，逻辑像素）。
///
/// 产品表格在 B5/B6 接入后由 UI 回填；本阶段 schema 先行，保证设置文件
/// 版本稳定。
pub(crate) type ColumnLayouts = BTreeMap<String, BTreeMap<String, u32>>;

/// 窗口尺寸（逻辑像素）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct WindowSize {
    /// 宽度。
    pub(crate) width: u32,
    /// 高度。
    pub(crate) height: u32,
}

impl WindowSize {
    /// 默认窗口尺寸（DESIGN.md §7：1280×800）。
    pub(crate) const DEFAULT: Self = Self {
        width: 1280,
        height: 800,
    };

    /// 最小窗口尺寸。
    pub(crate) const MIN: Self = Self {
        width: 960,
        height: 640,
    };

    /// 夹取到最小窗口之内（防止外部编辑出过小尺寸）。
    pub(crate) fn clamped(self) -> Self {
        Self {
            width: self.width.max(Self::MIN.width),
            height: self.height.max(Self::MIN.height),
        }
    }
}

/// 可持久化设置（allowlist 白名单，见模块文档）。
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Settings {
    /// 主题（`"light"` / `"dark"`；`None` 跟随系统默认浅色）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) theme: Option<String>,
    /// 界面语言（BCP 47 标签；`None` 用默认 en）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) language: Option<String>,
    /// 上次关闭时的窗口尺寸。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) window: Option<WindowSize>,
    /// 上次停留的工作区（稳定键名，见 `WorkspaceId::key`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) last_workspace: Option<String>,
    /// 各工作区的列布局。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) column_layouts: ColumnLayouts,
}

impl Settings {
    /// 解析主题字段；无法识别的取值回退默认浅色。
    pub(crate) fn theme_mode(&self) -> Option<gpui_component::ThemeMode> {
        self.theme.as_deref().map(|value| match value {
            "dark" => gpui_component::ThemeMode::Dark,
            _ => gpui_component::ThemeMode::Light,
        })
    }

    /// 解析语言字段；无法识别的取值回退默认 en。
    pub(crate) fn language(&self) -> Lang {
        self.language
            .as_deref()
            .and_then(Lang::parse)
            .unwrap_or_default()
    }

    /// 解析最后工作区；无法识别的键名回退默认 Processes。
    pub(crate) fn last_workspace(&self) -> runquiry_ui::WorkspaceId {
        self.last_workspace
            .as_deref()
            .and_then(runquiry_ui::WorkspaceId::parse)
            .unwrap_or_default()
    }
}

/// 从磁盘加载设置；文件不存在或内容损坏时返回默认值（不 panic、不报错打扰）。
///
/// 损坏回退是刻意选择：设置文件不是用户数据，宁可重置也不阻塞启动。
pub(crate) fn load_settings(path: &Path) -> Settings {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// 原子写入设置：先写临时文件再重命名，避免中途失败留下半截文件。
///
/// 目录不存在时先创建。失败时返回错误由调用方决定提示方式，设置写入失败
/// 不影响应用运行。
pub(crate) fn save_settings(path: &Path, settings: &Settings) -> Result<(), String> {
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    // 临时文件与目标同目录，保证 rename 在同一文件系统上是原子的。
    let temp = path.with_extension("json.tmp");
    {
        let mut file = std::fs::File::create(&temp).map_err(|e| e.to_string())?;
        file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
    }
    std::fs::rename(&temp, path).map_err(|e| e.to_string())
}

/// 默认设置文件路径（按平台标准位置组装；测试不要调用本函数）。
///
/// - Linux：`$XDG_CONFIG_HOME`（缺省 `$HOME/.config`）下的 `runquiry/settings.json`
/// - macOS：`$HOME/Library/Application Support/runquiry/settings.json`
/// - Windows：`%APPDATA%\runquiry\settings.json`
///
/// 环境变量缺失时回退系统临时目录：调用方（[`save_settings`]）承诺只写
/// 绝对路径，不把设置文件落进随进程 cwd 漂移的相对位置。
pub(crate) fn default_settings_path() -> PathBuf {
    if cfg!(target_os = "windows") {
        let base = env_base("APPDATA");
        return base.join("runquiry").join("settings.json");
    }
    if cfg!(target_os = "macos") {
        let base = env_base("HOME").join("Library").join("Application Support");
        return base.join("runquiry").join("settings.json");
    }
    // Linux/其他：XDG 规范。
    let base = std::env::var("XDG_CONFIG_HOME").ok().filter(|base| {
        // XDG 规范要求绝对路径；相对路径的 XDG_CONFIG_HOME 不采用。
        Path::new(base).is_absolute()
    });
    let base = base.map_or_else(|| env_base("HOME").join(".config"), PathBuf::from);
    base.join("runquiry").join("settings.json")
}

/// 读取基路径环境变量；缺失或为空时回退系统临时目录（保证结果为绝对路径）。
fn env_base(key: &str) -> PathBuf {
    let value = std::env::var(key).unwrap_or_default();
    if value.is_empty() {
        std::env::temp_dir()
    } else {
        PathBuf::from(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use runquiry_ui::WorkspaceId;

    /// 唯一可写的临时目录（测试不触碰真实用户目录）。
    fn temp_dir(name: &str) -> PathBuf {
        let base = std::env::temp_dir().join("runquiry-settings-test");
        std::fs::create_dir_all(&base).ok();
        base.join(name)
    }

    /// 空设置 == 默认值；round-trip 后语义不变。
    #[test]
    fn defaults_and_round_trip() {
        assert_eq!(Settings::default().theme_mode(), None);
        assert_eq!(Settings::default().language(), Lang::En);
        assert_eq!(Settings::default().last_workspace(), WorkspaceId::Processes);
        assert_eq!(Settings::default().window, None);

        let path = temp_dir("round-trip.json");
        std::fs::remove_file(&path).ok();
        assert_eq!(
            load_settings(&path),
            Settings::default(),
            "文件不存在用默认值"
        );

        let settings = Settings {
            theme: Some(String::from("dark")),
            language: Some(String::from("zh-CN")),
            window: Some(WindowSize {
                width: 1600,
                height: 900,
            }),
            last_workspace: Some(String::from("ports")),
            column_layouts: BTreeMap::new(),
        };
        save_settings(&path, &settings).ok();
        assert_eq!(load_settings(&path), settings, "恢复必须逐字段一致");
        std::fs::remove_file(&path).ok();
    }

    /// 损坏文件安全回退默认值，不 panic。
    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let path = temp_dir("corrupt.json");
        std::fs::write(&path, "{ not valid json !!!").ok();
        assert_eq!(load_settings(&path), Settings::default());
        std::fs::remove_file(&path).ok();

        // 合法 JSON 但 schema 不符（数组）同样回退，不得 panic。
        std::fs::write(&path, "[1, 2, 3]").ok();
        assert_eq!(load_settings(&path), Settings::default());
        std::fs::remove_file(&path).ok();
    }

    /// allowlist schema：未知字段被拒绝（回退默认），窗口尺寸被夹取到最小值。
    #[test]
    fn schema_rejects_unknown_fields() {
        let path = temp_dir("unknown-field.json");
        std::fs::write(&path, r#"{"theme":"dark","last_filter":"ssh -i"}"#).ok();
        // 未知字段 last_filter 即使内容像调查输入也不允许存在：整份文件拒绝。
        let settings = load_settings(&path);
        assert_eq!(settings, Settings::default());
        std::fs::remove_file(&path).ok();

        let path = temp_dir("clamp.json");
        std::fs::write(&path, r#"{"window":{"width":320,"height":200}}"#).ok();
        let settings = load_settings(&path);
        assert_eq!(
            settings.window.map(WindowSize::clamped),
            Some(WindowSize::MIN)
        );
        std::fs::remove_file(&path).ok();
    }

    /// 设置文件脱敏：结构里不存在任何调查输入字段（编译期保证 + 序列化断言）。
    #[test]
    fn settings_carry_no_inspection_data() {
        let text = serde_json::to_string(&Settings {
            theme: Some(String::from("dark")),
            language: Some(String::from("zh-CN")),
            window: Some(WindowSize {
                width: 1280,
                height: 800,
            }),
            last_workspace: Some(String::from("file-locks")),
            column_layouts: BTreeMap::new(),
        })
        .ok();
        let text = text.unwrap_or_default();
        for banned in ["pid", "target", "filter", "selection", "query", "path"] {
            assert!(
                !text.to_lowercase().contains(banned),
                "设置文件不得包含 {banned}"
            );
        }
    }

    /// 默认路径组装：Linux 用 XDG，无法解析 HOME 时得到可写路径也不 panic。
    #[test]
    fn default_path_is_prefixed_and_absolute() {
        let path = default_settings_path();
        assert!(path.is_absolute(), "默认设置路径必须是绝对路径: {path:?}");
        assert!(path.ends_with("settings.json"));
    }
}
