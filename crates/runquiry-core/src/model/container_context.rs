//! 容器身份上下文与 cgroup 解析纯函数（parity §1 容器身份字段 / §5、§7 容器识别）。
//!
//! 本模块只做 cgroup 文本的静态语义解析（对齐 witr `internal/source/container.go`
//! 与 `internal/proc/process_linux.go`）；Snap（`SNAP_NAME=`）/Flatpak
//! （`FLATPAK_ID=`）环境变量判定属来源判定链，不放这里。

use serde::{Deserialize, Serialize};

/// 容器健康检查状态（parity：`ContainerHealthcheck` 的 `present` / `absent`）。
///
/// 探测仅对 docker / podman 生效（parity §4），其余运行时或平台不可判定，
/// 由 `Option` 的 `None` 表达（对应 witr 空字符串）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthcheckStatus {
    /// 容器定义了 HEALTHCHECK。
    #[serde(rename = "present")]
    Present,
    /// 容器未定义 HEALTHCHECK。
    #[serde(rename = "absent")]
    Absent,
}

/// 容器身份上下文（parity：`Process.ContainerID / ContainerRuntime /
/// ContainerHealthcheck`，取自 Linux cgroup 检测）。
///
/// 语义锚点为 witr 的零值约定：cgroup 中未携带 ID 的分支（colima 默认实例、
/// LXC payload 名称以外的场景）`container_id` 为空串；witr 未标注运行时的
/// 分支（colima、LXC 系）`runtime` 以来源名填充，保证非空可用于解析容器名。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerContext {
    /// 容器 ID（长 ID 优先；LXC 分支为 payload 容器名）。
    container_id: String,
    /// 运行时标识（witr `containerRuntime`：docker / podman / crictl / nerdctl；
    /// colima 与 LXC 系为来源名 colima / lxc）。
    runtime: String,
    /// 健康检查定义状态；不可判定为 `None`。
    healthcheck: Option<HealthcheckStatus>,
}

impl ContainerContext {
    /// 构造容器上下文。
    pub const fn new(
        container_id: String,
        runtime: String,
        healthcheck: Option<HealthcheckStatus>,
    ) -> Self {
        Self {
            container_id,
            runtime,
            healthcheck,
        }
    }

    /// 读取容器 ID（空串表示 cgroup 未携带 ID）。
    pub fn container_id(&self) -> &str {
        &self.container_id
    }

    /// 读取运行时标识。
    pub fn runtime(&self) -> &str {
        &self.runtime
    }

    /// 读取健康检查定义状态。
    pub const fn healthcheck(&self) -> Option<HealthcheckStatus> {
        self.healthcheck
    }

    /// 改写运行时标识（管线做容器标签一致性改写时使用；additive 最小面）。
    pub fn set_runtime(&mut self, runtime: &str) {
        self.runtime = String::from(runtime);
    }

    /// 写回健康检查探测结果（管线补全步骤使用；additive 最小面）。
    pub const fn set_healthcheck(&mut self, status: Option<HealthcheckStatus>) {
        self.healthcheck = status;
    }
}

/// 从单份 `/proc/PID/cgroup` 原文判定容器上下文。
///
/// 分支与优先级对齐 witr `detectContainer`（docker → podman/libpod → kubepods
/// → colima → containerd → lxc.payload，命中即返回）；runtime 取值对齐 witr
/// `process_linux.go` 的 `containerRuntime` 赋值（kubepods → `crictl`、
/// containerd → `nerdctl`）。LXC 分支的具体运行时（incus / lxd / lxc）由调用方
/// 以 [`detect_lxc_runtime`] 按祖先命令覆盖。
///
/// 环境变量判定（Snap / Flatpak）不在此函数：cgroup 判不出时由来源判定链读取
/// 目标进程环境。
pub fn detect_container_from_cgroup(content: &str) -> Option<ContainerContext> {
    if content.contains("docker") {
        Some(context_with_extracted_id(
            content, "docker", "docker-", "docker/",
        ))
    } else if content.contains("podman") || content.contains("libpod") {
        Some(context_with_extracted_id(
            content, "podman", "libpod-", "libpod/",
        ))
    } else if content.contains("kubepods") {
        Some(ContainerContext::new(
            find_long_hex_id(content).unwrap_or_default(),
            String::from("crictl"),
            None,
        ))
    } else if content.contains("colima") {
        Some(ContainerContext::new(
            colima_scope_id(content).unwrap_or_default(),
            String::from("colima"),
            None,
        ))
    } else if content.contains("containerd") {
        Some(ContainerContext::new(
            find_long_hex_id(content).unwrap_or_default(),
            String::from("nerdctl"),
            None,
        ))
    } else if content.contains("lxc.payload") {
        Some(ContainerContext::new(
            lxc_payload_name(content),
            String::from("lxc"),
            None,
        ))
    } else {
        None
    }
}

/// 以 scope / 路径两种 cgroup 模式提取 ID 并组装上下文（parity：
/// `extractContainerID`；`prefix-<id>.scope` 优先，`prefix/<64hex>` 兜底，
/// 与 witr 一致不做十六进制校验）。
fn context_with_extracted_id(
    content: &str,
    runtime: &str,
    dash_prefix: &str,
    slash_prefix: &str,
) -> ContainerContext {
    let container_id = extract_container_id(content, dash_prefix, slash_prefix).unwrap_or_default();
    ContainerContext::new(container_id, String::from(runtime), None)
}

/// scope / 路径两种 cgroup 模式的 ID 提取（parity：`extractContainerID`）。
fn extract_container_id(content: &str, dash_prefix: &str, slash_prefix: &str) -> Option<String> {
    // 模式 1：.../prefix-<id>.scope
    if let Some(idx) = content.find(dash_prefix) {
        let rest = &content[idx + dash_prefix.len()..];
        if let Some(dot) = rest.find(".scope") {
            return Some(String::from(&rest[..dot]));
        }
    }
    // 模式 2：.../prefix/<64hex>
    if let Some(idx) = content.find(slash_prefix) {
        let rest = &content[idx + slash_prefix.len()..];
        if let Some(id) = rest.get(..64) {
            return Some(String::from(id));
        }
    }
    None
}

/// 提取 colima scope 内的实例 ID（`colima-<id>.scope`）。
fn colima_scope_id(content: &str) -> Option<String> {
    let idx = content.find("colima-")?;
    let rest = &content[idx + "colima-".len()..];
    let dot = rest.find(".scope")?;
    Some(String::from(&rest[..dot]))
}

/// 提取 LXC payload 容器名（parity：`extractLXCBasedContainerName`；
/// 仅剥 `user-<uid>_` 前缀，不剥任意下划线，截断到首个 `/`）。
fn lxc_payload_name(content: &str) -> String {
    let Some(idx) = content.find("lxc.payload.") else {
        return String::new();
    };
    let mut rest = &content[idx + "lxc.payload.".len()..];
    if rest.starts_with("user-")
        && let Some(u) = rest.find('_')
    {
        rest = &rest[u + 1..];
    }
    rest.find('/')
        .map_or_else(|| String::from(rest), |slash| String::from(&rest[..slash]))
}

/// 在文本中查找首个 64 位十六进制长 ID（parity：`findLongHexID`，
/// 0-9a-fA-F 逐字符校验，返回首个 64 字符窗口）。
///
/// 非字符边界处不切片，避免多字节 UTF-8 输入触发 panic。
pub fn find_long_hex_id(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut start = 0usize;
    while start + 64 <= bytes.len() {
        let window = &bytes[start..start + 64];
        if window.iter().all(u8::is_ascii_hexdigit) {
            return text.get(start..start + 64).map(String::from);
        }
        start += 1;
    }
    None
}

/// 长 ID 截短为 12 字符（parity：`shortID`；不足 12 字符原样返回）。
pub fn short_id(long: &str) -> &str {
    if long.len() > 12 && long.is_char_boundary(12) {
        &long[..12]
    } else {
        long
    }
}

/// 依据祖先命令判定 LXC 系运行时（parity：`detectLXCRuntime`）。
///
/// witr 按祖先链逐个比对命令名（`incusd` / `lxd` / `lxc-start`），本函数接收
/// 单个命令名，由调用方在祖先链上迭代；无命中回退 `"lxc"`。
/// witr 匹配的是守护进程名 `incusd`（非 `incus`），回退值恒为 `"lxc"`。
pub fn detect_lxc_runtime(ancestor_command: &str) -> &'static str {
    match ancestor_command {
        "incusd" => "incus",
        "lxd" => "lxd",
        // lxc-start 与未命中回退同为 "lxc"（witr switch 的显式分支与 fallback 值相同）。
        _ => "lxc",
    }
}

/// 从单份 `/proc/PID/cgroup` 原文解析所属 systemd 单元名（parity：
/// `getUnitNameFromCgroup`，cgroup v1 `name=systemd` controller 行与 v2 空行均
/// 可命中；从路径末端找首个 `.service` / `.scope` 结尾的段）。
pub fn systemd_unit_from_cgroup(content: &str) -> Option<String> {
    for line in content.lines() {
        let mut parts = line.splitn(3, ':');
        let _hierarchy = parts.next();
        let Some(controllers) = parts.next() else {
            continue;
        };
        let Some(path) = parts.next() else {
            continue;
        };
        if !(controllers.is_empty() || controllers.contains("systemd")) {
            continue;
        }
        for part in path.trim().split('/').rev() {
            // 大小写敏感的后缀比较，与 witr ends_with 语义逐字一致；
            // 经 rsplit_once 表达以规避对文件扩展名比较的误判建议。
            if matches!(part.rsplit_once('.'), Some((_, "service" | "scope"))) {
                return Some(String::from(part));
            }
        }
    }
    None
}

/// 从单份 `/proc/PID/cgroup` 原文解析进程所属的 systemd **服务**单元名
/// （parity：`serviceUnitFromCgroup`，仅供 `Service` 字段与告警比对）。
///
/// 与 [`systemd_unit_from_cgroup`] 的差异：最近单元是 `.scope`（会话/应用
/// 作用域）时返回 `None`——scope 不是受管服务；仅 `.service` 命中。
pub fn service_unit_from_cgroup(content: &str) -> Option<String> {
    for line in content.lines() {
        let mut parts = line.splitn(3, ':');
        let _hierarchy = parts.next();
        let Some(controllers) = parts.next() else {
            continue;
        };
        let Some(path) = parts.next() else {
            continue;
        };
        if !(controllers.is_empty() || controllers.contains("systemd")) {
            continue;
        }
        for part in path.trim().split('/').rev() {
            if matches!(part.rsplit_once('.'), Some((_, "service"))) {
                return Some(String::from(part));
            }
            if matches!(part.rsplit_once('.'), Some((_, "scope"))) {
                return None; // 最近单元是 scope：非受管服务
            }
        }
    }
    None
}
