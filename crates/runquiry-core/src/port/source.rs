//! 来源识别的证据采集端口：平台只采集原始证据，core 做全部解释。

use std::fmt;

use crate::model::ids::Pid;
use crate::model::process::ProcessSummary;

/// 平台侧采集的来源识别原始证据。
///
/// 平台实现（B2 各平台采集器）**不做任何来源类型判定**：本结构只承载
/// `/proc`、环境变量与 D-Bus 的原始文本，来源判定（优先级链、cgroup 分支、
/// SSH / shell / systemd 等识别）全部由 core 依据
/// [`SourceEvidenceProvider::evidence`] 的返回值完成。
///
/// 敏感性与日志约定：[`SourceEvidence::env_by_pid`] 携带的原始环境变量值
/// 不得写入日志或诊断消息（采集即脱敏边界在平台侧——不打日志即可，core
/// 呈现层再统一脱敏）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceEvidence {
    /// 目标及祖先链各 PID 的 `/proc/PID/cgroup` 原文（容器与 systemd 单元
    /// 判定的输入）；读不到（权限、已退出、非 Linux）的 PID 省略。
    pub cgroup_by_pid: Vec<(Pid, String)>,
    /// 回溯环境变量用的原始键值（按 PID；目标优先、祖先链回溯的输入）。
    /// 值不得写入日志；Snap（`SNAP_NAME=`）/Flatpak（`FLATPAK_ID=`）容器
    /// 判定与 SSH 连接详情回溯均消费本字段。
    pub env_by_pid: Vec<(Pid, Vec<(String, String)>)>,
    /// systemd 是否正在运行（Linux 仅读总线可得性；其他平台恒 `false`）。
    pub systemd_running: bool,
    /// systemd D-Bus 富化键值（`Description` / `FragmentPath` / `SourcePath` /
    /// `NRestarts` / 定时器 schedule 等，best-effort）；core 不解释语义，
    /// 原样透传进 `Source.details`。
    pub systemd_details: Vec<(String, String)>,
    /// launchd 托管证据（仅 macOS）：launchd 管理的 PID → 原始键值。
    /// 键契约（core 按这些键组装 [`crate::model::source::Source`]，未取得的
    /// 键省略）：`label`（launchd label，写入 `Source.name`）、`comment`
    /// （写入 `Source.description`）、`plist`（plist 路径，写入
    /// `Source.unit_file` 与 details）、`type`（Launch Agent/Daemon 域描述）、
    /// `schedule` / `triggers`（触发器文本，core 原样透传）、`keepalive`
    /// （`KeepAlive` 状态文本）。判定本身（祖先链含 PID 1 `launchd` 等）由 core
    /// 完成，平台不判定来源类型。
    pub launchd_by_pid: Vec<(Pid, Vec<(String, String)>)>,
    /// Windows SCM 服务证据（仅 Windows）：PID → 原始键值。键契约：
    /// `service`（SCM 服务名，写入 `Source.name`）、`description`
    /// （写入 `Source.description`）、`display_name` / `start_mode` /
    /// `binary_path` / `state`（core 原样透传）。core 依据 witr `detectWindowsService`
    /// 的三级判定解释这些键值，平台不判定来源类型。
    pub windows_service_by_pid: Vec<(Pid, Vec<(String, String)>)>,
}

/// 来源证据采集端口（同步）。
///
/// 前置条件：`ancestry` 为根→目标顺序的祖先链快照（pipeline 已解析，可能
/// 因部分成功而截断）；实现按其中的 PID（必要时含父 PID）读取证据。
///
/// 后置条件：
/// * 返回的 [`SourceEvidence`] 只含原始采集值，不含判定结果；
/// * 单项读取失败（如某 PID 的 cgroup 读不到）就地省略，不产生错误，也
///   不得伪造占位内容；
/// * 平台不得自行判定来源类型——判定是 core 的唯一职责。
pub trait SourceEvidenceProvider: fmt::Debug {
    /// 采集来源识别所需的原始证据。
    fn evidence(&self, ancestry: &[ProcessSummary]) -> SourceEvidence;
}
