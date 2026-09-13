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

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fs::{File, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _};

use serde::{Deserialize, Serialize};

use runquiry_ui::Lang;

/// 同进程内临时文件序号；配合 PID 与排他创建避免保存间互相覆盖。
static SETTINGS_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
/// 遇到同 PID 的崩溃残留时继续尝试后续唯一名。
const TEMP_CREATE_ATTEMPTS: usize = 128;

/// 列布局：工作区键 →（列 ID → 列宽，逻辑像素）。
///
/// 产品表格在 B5/B6 接入后由 UI 回填；本阶段 schema 先行，保证设置文件
/// 版本稳定。
pub(crate) type ColumnLayouts = BTreeMap<String, BTreeMap<String, u32>>;

/// 各工作区隐藏的表格列：工作区键 → 列 ID 集合（列显隐设置按钮写入）。
pub(crate) type HiddenColumns = BTreeMap<String, BTreeSet<String>>;

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
    /// 各工作区隐藏的表格列（列显隐设置按钮持久化；列 ID 为表格列 key，
    /// 非调查输入）。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) hidden_columns: HiddenColumns,
}

impl Settings {
    /// 解析主题字段；无法识别的取值回退默认浅色。
    pub(crate) fn theme_mode(&self) -> Option<gpui_kit::component::ThemeMode> {
        self.theme.as_deref().map(|value| match value {
            "dark" => gpui_kit::component::ThemeMode::Dark,
            _ => gpui_kit::component::ThemeMode::Light,
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
pub(crate) fn load_settings(path: Option<&Path>) -> Settings {
    let Some(path) = path.filter(|path| path.is_absolute()) else {
        return Settings::default();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Settings::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

/// 原子写入设置：先写临时文件再重命名，避免中途失败留下半截文件。
///
/// 目录不存在时先创建。失败时返回错误由调用方决定提示方式，设置写入失败
/// 不影响应用运行。
pub(crate) fn save_settings(path: Option<&Path>, settings: &Settings) -> Result<(), String> {
    let Some(path) = path else {
        return Ok(());
    };
    if !path.is_absolute() {
        return Err(String::from("设置文件路径必须是绝对路径"));
    }
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    let parent = path
        .parent()
        .ok_or_else(|| String::from("设置文件路径缺少父目录"))?;
    create_settings_directory(parent).map_err(|e| e.to_string())?;

    // 临时文件与目标同目录，保证 rename 在同一文件系统上是原子的。
    // 排他创建不跟随预置符号链接；Unix 新文件权限固定为 0600。
    let (temp, mut file) = create_temp_file(path).map_err(|e| e.to_string())?;
    if let Err(error) = file
        .write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
    {
        drop(file);
        return Err(error_with_temp_cleanup(&temp, &error));
    }
    drop(file);
    std::fs::rename(&temp, path).map_err(|error| error_with_temp_cleanup(&temp, &error))
}

/// 创建缺失的设置目录；Unix 新目录限制为当前用户可访问。
fn create_settings_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true).mode(0o700).create(path)
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(path)
    }
}

/// 在目标文件旁排他创建唯一临时文件。
fn create_temp_file(target: &Path) -> io::Result<(PathBuf, File)> {
    let parent = target
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "设置文件路径缺少父目录"))?;
    let file_name = target
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "设置文件路径缺少文件名"))?;

    for _ in 0..TEMP_CREATE_ATTEMPTS {
        let sequence = SETTINGS_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let mut temp_name = OsString::from(".");
        temp_name.push(file_name);
        temp_name.push(format!(".runquiry-{}-{sequence}.tmp", std::process::id()));
        let temp = parent.join(temp_name);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        match options.open(&temp) {
            Ok(file) => return Ok((temp, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "无法分配唯一设置临时文件",
    ))
}

/// 合并主错误与临时文件清理错误；只清理本次排他创建的路径。
fn error_with_temp_cleanup(temp: &Path, error: &io::Error) -> String {
    match std::fs::remove_file(temp) {
        Ok(()) => error.to_string(),
        Err(cleanup_error) => format!("{error}; 清理设置临时文件失败: {cleanup_error}"),
    }
}

/// 从环境基路径组装设置路径；相对或缺失的基路径一律拒绝。
fn settings_path_from_base(base: Option<&OsStr>, suffix: &[&str]) -> Option<PathBuf> {
    let mut path = absolute_base(base)?;
    for component in suffix {
        path.push(component);
    }
    Some(path.join("runquiry").join("settings.json"))
}

/// 把环境输入解析成绝对基路径。
fn absolute_base(value: Option<&OsStr>) -> Option<PathBuf> {
    value.map(PathBuf::from).filter(|path| path.is_absolute())
}

/// 默认设置文件路径（按平台标准位置组装；测试不要调用本函数）。
///
/// - Linux：`$XDG_CONFIG_HOME`（缺省 `$HOME/.config`）下的 `runquiry/settings.json`
/// - Windows：`%APPDATA%\runquiry\settings.json`
///
/// 环境变量缺失、为空或为相对路径时返回 `None`，由调用方安全禁用持久化。
pub(crate) fn default_settings_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var_os("APPDATA");
        settings_path_from_base(appdata.as_deref(), &[])
    }
    #[cfg(not(target_os = "windows"))]
    {
        let xdg_config_home = std::env::var_os("XDG_CONFIG_HOME");
        let home = std::env::var_os("HOME");
        settings_path_from_base(xdg_config_home.as_deref(), &[])
            .or_else(|| settings_path_from_base(home.as_deref(), &[".config"]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use runquiry_ui::WorkspaceId;
    use std::io;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    /// 唯一可写的临时目录（测试不触碰真实用户目录）。
    fn temp_dir(name: &str) -> Result<PathBuf, String> {
        for _ in 0..128 {
            let sequence = TEST_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "runquiry-settings-test-{}-{sequence}-{name}",
                std::process::id()
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(path),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.to_string()),
            }
        }
        Err(String::from("无法分配唯一测试目录"))
    }

    fn io_result<T>(result: io::Result<T>) -> Result<T, String> {
        result.map_err(|error| error.to_string())
    }

    /// 空设置 == 默认值；round-trip 后语义不变。
    #[test]
    fn defaults_and_round_trip() -> Result<(), String> {
        assert_eq!(Settings::default().theme_mode(), None);
        assert_eq!(Settings::default().language(), Lang::En);
        assert_eq!(Settings::default().last_workspace(), WorkspaceId::Processes);
        assert_eq!(Settings::default().window, None);

        let directory = temp_dir("round-trip")?;
        let path = directory.join("settings.json");
        assert_eq!(
            load_settings(Some(&path)),
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
            hidden_columns: BTreeMap::from([(
                String::from("processes"),
                BTreeSet::from([String::from("health")]),
            )]),
        };
        save_settings(Some(&path), &settings)?;
        assert_eq!(load_settings(Some(&path)), settings, "恢复必须逐字段一致");
        io_result(std::fs::remove_dir_all(directory))
    }

    /// 损坏文件安全回退默认值，不 panic。
    #[test]
    fn corrupt_file_falls_back_to_defaults() -> Result<(), String> {
        let directory = temp_dir("corrupt")?;
        let path = directory.join("settings.json");
        io_result(std::fs::write(&path, "{ not valid json !!!"))?;
        assert_eq!(load_settings(Some(&path)), Settings::default());

        // 合法 JSON 但 schema 不符（数组）同样回退，不得 panic。
        io_result(std::fs::write(&path, "[1, 2, 3]"))?;
        assert_eq!(load_settings(Some(&path)), Settings::default());
        io_result(std::fs::remove_dir_all(directory))
    }

    /// allowlist schema：未知字段被拒绝（回退默认），窗口尺寸被夹取到最小值。
    #[test]
    fn schema_rejects_unknown_fields() -> Result<(), String> {
        let directory = temp_dir("schema")?;
        let path = directory.join("unknown-field.json");
        io_result(std::fs::write(
            &path,
            r#"{"theme":"dark","last_filter":"ssh -i"}"#,
        ))?;
        // 未知字段 last_filter 即使内容像调查输入也不允许存在：整份文件拒绝。
        let settings = load_settings(Some(&path));
        assert_eq!(settings, Settings::default());

        let path = directory.join("clamp.json");
        io_result(std::fs::write(
            &path,
            r#"{"window":{"width":320,"height":200}}"#,
        ))?;
        let settings = load_settings(Some(&path));
        assert_eq!(
            settings.window.map(WindowSize::clamped),
            Some(WindowSize::MIN)
        );
        io_result(std::fs::remove_dir_all(directory))
    }

    /// 设置文件脱敏：结构里不存在任何调查输入字段（编译期保证 + 序列化断言）。
    #[test]
    fn settings_carry_no_inspection_data() -> Result<(), String> {
        let text = serde_json::to_string(&Settings {
            theme: Some(String::from("dark")),
            language: Some(String::from("zh-CN")),
            window: Some(WindowSize {
                width: 1280,
                height: 800,
            }),
            last_workspace: Some(String::from("file-locks")),
            column_layouts: BTreeMap::new(),
            hidden_columns: BTreeMap::new(),
        })
        .map_err(|error| error.to_string())?;
        for banned in ["pid", "target", "filter", "selection", "query", "path"] {
            assert!(
                !text.to_lowercase().contains(banned),
                "设置文件不得包含 {banned}"
            );
        }
        Ok(())
    }

    /// 旧固定临时文件即使被预置为符号链接，也不得覆盖链接目标。
    #[cfg(unix)]
    #[test]
    fn save_does_not_follow_legacy_temp_symlink() -> Result<(), String> {
        use std::os::unix::fs::symlink;

        let directory = temp_dir("legacy-temp-symlink")?;
        let path = directory.join("settings.json");
        let victim = directory.join("victim.txt");
        let legacy_temp = path.with_extension("json.tmp");
        io_result(std::fs::write(&victim, "do-not-touch"))?;
        io_result(symlink(&victim, &legacy_temp))?;

        let settings = Settings {
            theme: Some(String::from("dark")),
            ..Settings::default()
        };
        save_settings(Some(&path), &settings)?;

        assert_eq!(io_result(std::fs::read_to_string(&victim))?, "do-not-touch");
        assert_eq!(load_settings(Some(&path)), settings);
        io_result(std::fs::remove_dir_all(directory))
    }

    /// 已有设置文件可以由后续保存原子替换。
    #[test]
    fn repeated_save_updates_existing_settings() -> Result<(), String> {
        let directory = temp_dir("repeated-save")?;
        let path = directory.join("settings.json");
        let first = Settings {
            theme: Some(String::from("light")),
            ..Settings::default()
        };
        let second = Settings {
            theme: Some(String::from("dark")),
            ..Settings::default()
        };

        save_settings(Some(&path), &first)?;
        save_settings(Some(&path), &second)?;

        assert_eq!(load_settings(Some(&path)), second);
        io_result(std::fs::remove_dir_all(directory))
    }

    /// 无安全配置路径时，加载使用默认值且保存明确成为无副作用操作。
    #[test]
    fn disabled_persistence_is_a_no_op() -> Result<(), String> {
        let settings = Settings {
            theme: Some(String::from("dark")),
            ..Settings::default()
        };

        save_settings(None, &settings)?;

        assert_eq!(load_settings(None), Settings::default());
        Ok(())
    }

    /// Unix 上新设置目录与文件均仅允许当前用户访问。
    #[cfg(unix)]
    #[test]
    fn saved_settings_have_private_permissions() -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = temp_dir("private-permissions")?;
        let settings_directory = directory.join("runquiry");
        let path = settings_directory.join("settings.json");
        save_settings(Some(&path), &Settings::default())?;

        let directory_mode = io_result(std::fs::metadata(&settings_directory))?
            .permissions()
            .mode()
            & 0o777;
        let file_mode = io_result(std::fs::metadata(&path))?.permissions().mode() & 0o777;
        assert_eq!(directory_mode, 0o700);
        assert_eq!(file_mode, 0o600);
        io_result(std::fs::remove_dir_all(directory))
    }

    /// Linux/XDG 输入均为相对路径时，不得生成随 cwd 漂移的配置路径。
    #[test]
    fn linux_relative_bases_disable_persistence() {
        let xdg_path = settings_path_from_base(Some(std::ffi::OsStr::new("relative-xdg")), &[]);
        let home_path =
            settings_path_from_base(Some(std::ffi::OsStr::new("relative-home")), &[".config"]);

        assert_eq!(xdg_path, None);
        assert_eq!(home_path, None);
    }

    /// Windows 的相对 APPDATA 不得用于生产配置。
    #[test]
    fn windows_relative_appdata_disables_persistence() {
        let path = settings_path_from_base(Some(std::ffi::OsStr::new("relative-appdata")), &[]);

        assert_eq!(path, None);
    }

    /// 所有候选环境变量缺失时，不回退共享临时目录或 cwd。
    #[test]
    fn missing_environment_disables_persistence() {
        assert_eq!(settings_path_from_base(None, &[]), None);
        assert_eq!(settings_path_from_base(None, &[".config"]), None);
        assert_eq!(
            settings_path_from_base(None, &["Library", "Application Support"]),
            None
        );
    }

    /// 默认路径存在时必须是绝对路径并以 settings.json 结尾。
    #[test]
    fn default_path_is_prefixed_and_absolute() {
        if let Some(path) = default_settings_path() {
            assert!(path.is_absolute(), "默认设置路径必须是绝对路径: {path:?}");
            assert!(path.ends_with("settings.json"));
        }
    }
}
