//! 进程启动来源模型（parity §1 `Source` / §5 来源识别优先级链）。

use serde::{Deserialize, Serialize};

/// 来源类型（parity：witr `SourceType` 常量集，序列化值与 witr 字符串一致）。
///
/// 来源识别按固定优先级链依次判定（parity §5）：
/// container → ssh → shell → systemd → launchd → BSD rc → supervisor → cron
/// → Windows service → init，全部未命中则返回 [`SourceType::Unknown`]——
/// 来源识别永不返回空。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceType {
    /// 容器（docker / podman / kubernetes / colima / containerd / LXC 系 / Snap / Flatpak）。
    #[serde(rename = "container")]
    Container,
    /// SSH 会话（祖先链存在 sshd）。
    #[serde(rename = "ssh")]
    Ssh,
    /// 交互 shell 或用户工具（bash/zsh/python/node 等）。
    #[serde(rename = "shell")]
    Shell,
    /// systemd 服务（仅 Linux）。
    #[serde(rename = "systemd")]
    Systemd,
    /// launchd 服务（仅 macOS）。
    #[serde(rename = "launchd")]
    Launchd,
    /// BSD rc 脚本（witr 码为 `bsdrc`）。
    #[serde(rename = "bsdrc")]
    BsdRc,
    /// supervisor 托管进程。
    #[serde(rename = "supervisor")]
    Supervisor,
    /// cron / 定时任务。
    #[serde(rename = "cron")]
    Cron,
    /// Windows SCM 服务（仅 Windows）。
    #[serde(rename = "windows_service")]
    WindowsService,
    /// init 兜底（根进程 PID 1 / Windows PID 4 "System"）。
    #[serde(rename = "init")]
    Init,
    /// unknown 兜底：平台限制、权限错误、部分结果与空集合由此区分。
    #[serde(rename = "unknown")]
    Unknown,
}

impl SourceType {
    /// 稳定的来源码字符串，与 witr `SourceType` 字符串逐字一致
    /// （也等于本枚举的 serde 序列化值）。
    pub const fn code(self) -> &'static str {
        match self {
            Self::Container => "container",
            Self::Ssh => "ssh",
            Self::Shell => "shell",
            Self::Systemd => "systemd",
            Self::Launchd => "launchd",
            Self::BsdRc => "bsdrc",
            Self::Supervisor => "supervisor",
            Self::Cron => "cron",
            Self::WindowsService => "windows_service",
            Self::Init => "init",
            Self::Unknown => "unknown",
        }
    }
}

/// 进程启动来源（parity：`Source{Type, Name, Description, UnitFile, Details}`）。
///
/// 与 witr 的差异（intentional change）：
/// * `name` / `description` / `unit_file` 用 `Option<String>` 表达「未取得」，
///   替代 witr 的空字符串零值；
/// * `details` 用有序键值对列表替代 witr 的 map，保证 core 只透传平台采集
///   顺序、序列化顺序确定。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    /// 来源类型（witr `Source.Type`）。
    kind: SourceType,
    /// 来源实例名（如 `docker`、`nginx.service`、launchd label）；未取得为 `None`。
    name: Option<String>,
    /// 面向用户的来源描述（如 systemd D-Bus `Description`）；未取得为 `None`。
    description: Option<String>,
    /// 单元文件路径（systemd `UnitFile`；macOS 为 plist 路径、Windows 为 SCM
    /// 语义）；未取得为 `None`。
    unit_file: Option<String>,
    /// 来源附属详情（有序键值，core 只透传，不解释语义）。
    details: Vec<(String, String)>,
}

impl Source {
    /// 以来源类型构造（其余字段为空）。
    pub const fn new(source_type: SourceType) -> Self {
        Self {
            kind: source_type,
            name: None,
            description: None,
            unit_file: None,
            details: Vec::new(),
        }
    }

    /// unknown 兜底来源（parity：Detect 永不返回空）。
    pub const fn unknown() -> Self {
        Self::new(SourceType::Unknown)
    }

    /// 设置来源实例名（builder 风格，供来源判定链组装）。
    #[must_use]
    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    /// 设置来源描述。
    #[must_use]
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// 设置单元文件路径。
    #[must_use]
    pub fn with_unit_file(mut self, unit_file: String) -> Self {
        self.unit_file = Some(unit_file);
        self
    }

    /// 追加一条详情键值（保持采集顺序）。
    #[must_use]
    pub fn with_detail(mut self, key: String, value: String) -> Self {
        self.details.push((key, value));
        self
    }

    /// 读取来源类型。
    pub const fn source_type(&self) -> SourceType {
        self.kind
    }

    /// 读取来源实例名。
    pub const fn name(&self) -> Option<&str> {
        match &self.name {
            Some(name) => Some(name.as_str()),
            None => None,
        }
    }

    /// 读取来源描述。
    pub const fn description(&self) -> Option<&str> {
        match &self.description {
            Some(description) => Some(description.as_str()),
            None => None,
        }
    }

    /// 读取单元文件路径。
    pub const fn unit_file(&self) -> Option<&str> {
        match &self.unit_file {
            Some(unit_file) => Some(unit_file.as_str()),
            None => None,
        }
    }

    /// 读取有序详情键值列表。
    pub fn details(&self) -> &[(String, String)] {
        &self.details
    }
}
