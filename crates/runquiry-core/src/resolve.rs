//! 五类目标的解析纯函数（parity §2/§3）。
//!
//! 全部输入为已采集的类型化数据（进程清单、开放端口、容器清单、文件持有者）；
//! 字符串→目标值的边界解析见 [`crate::resolution`]。多结果不自动选择：
//! 命中多个时返回 [`Resolution::Ambiguous`] 与完整、稳定排序的候选集合。

use std::path::Path;

use crate::model::error::InspectError;
use crate::model::ids::{ContainerKey, Pid, Port};
use crate::model::process::ProcessSummary;
use crate::port::container::ContainerSummary;
use crate::port::file::FileInventoryEntry;
use crate::port::network::{OpenPortEntry, Protocol, SocketEntry};
use crate::resolution::{Resolution, matches_exact_token};

/// 名称扫描候选（parity：`ResolveName` 的 /proc 扫描段）。
///
/// 匹配规则（每条进程先 comm 后完整命令行，命中即计入该进程一次）：
/// * fuzzy：大小写不敏感子串（两侧转小写后 `contains`）；
/// * exact：comm 全等，或完整命令行做完整 token / 路径段匹配
///   （[`crate::matches_exact_token`]，两侧小写化在本函数内完成）；
/// * 纯数字守卫：查询串（小写后）等于该进程 PID 的十进制串时跳过该进程
///   （parity：`lowerName == strconv.Itoa(pid)` 时 continue，只作用于该 PID 自身）；
/// * `ignored` 中的 PID（调用方注入的自身 + 祖先链）被排除。
///
/// 返回去重后按 PID 升序的候选；无结果为空列表（是否回退 systemd 服务解析
/// 由调用方决定，见 [`merge_service_pid`] 与 [`resolve_name`]）。
#[must_use]
pub fn scan_name_candidates(
    inventory: &[ProcessSummary],
    query: &str,
    exact: bool,
    ignored: &[Pid],
) -> Vec<Pid> {
    let lower = query.to_lowercase();
    let mut pids: Vec<Pid> = inventory
        .iter()
        .filter(|p| !ignored.contains(&p.identity.pid()))
        .filter(|p| p.identity.pid().get().to_string() != lower)
        .filter(|p| {
            // 先 comm：exact 全等 / fuzzy 子串。
            let comm_lower = p.command.to_lowercase();
            if exact {
                comm_lower == lower
            } else {
                comm_lower.contains(&lower)
            }
        })
        .map(|p| p.identity.pid())
        .collect();
    // comm 未命中的进程再比完整命令行（parity：comm 命中即 continue，不再比 cmdline）。
    for p in inventory {
        if pids.contains(&p.identity.pid()) || ignored.contains(&p.identity.pid()) {
            continue;
        }
        if p.identity.pid().get().to_string() == lower {
            continue;
        }
        let Some(cmdline) = p.command_line.as_ref() else {
            continue;
        };
        let cmd_lower = cmdline.to_lowercase();
        let matched = if exact {
            matches_exact_token(&lower, &cmd_lower)
        } else {
            cmd_lower.contains(&lower)
        };
        if matched {
            pids.push(p.identity.pid());
        }
    }
    pids.sort_unstable();
    pids.dedup();
    pids
}

/// 把 systemd 服务解析得到的 PID 合并进扫描候选（parity：服务 PID 排首位，
/// 与扫描结果去重，其余按 PID 升序）。
///
/// witr 仅在 /proc 扫描零命中时才回退 systemctl；何时发起服务解析属调用方
/// （平台/上层），本函数只做确定性合并。
#[must_use]
pub fn merge_service_pid(service_pid: Option<Pid>, scanned: &[Pid]) -> Vec<Pid> {
    let mut rest: Vec<Pid> = scanned
        .iter()
        .filter(|p| Some(**p) != service_pid)
        .copied()
        .collect();
    rest.sort_unstable();
    rest.dedup();
    match service_pid {
        Some(service) => {
            let mut merged = vec![service];
            merged.extend(rest);
            merged
        }
        None => rest,
    }
}

/// 名称解析：扫描 + 可选服务 PID 合并 → [`Resolution`]。
///
/// `service_pid` 仅在扫描零命中时参与（parity：`len(procPIDs) == 0` 才回退
/// systemd）；无任何结果返回 [`InspectError::NotFound`]。
///
/// # Errors
/// 扫描与服务解析均无结果时返回 [`InspectError::NotFound`]。
pub fn resolve_name(
    inventory: &[ProcessSummary],
    query: &str,
    exact: bool,
    ignored: &[Pid],
    service_pid: Option<Pid>,
) -> Result<Resolution<Pid>, InspectError> {
    let scanned = scan_name_candidates(inventory, query, exact, ignored);
    // parity：服务 PID 仅在扫描零命中时参与，且排首位。
    let pids = if scanned.is_empty() {
        merge_service_pid(service_pid, &scanned)
    } else {
        scanned
    };
    resolution_from_pids(pids, format!("名称 {query:?}"))
}

/// 端口属主解析（`OpenPortEntry` 输入，parity §2 `ResolvePort` + 哨兵
/// `ErrSocketOwnerUnknown`）。
///
/// 无该端口的条目 → `NotFound`；有条目但属主全部不可知 → `SocketOwnerUnknown`
/// （调用方可走容器回退或提示提权）；多属主 → Ambiguous（升序去重）。
///
/// # Errors
/// 见函数说明。
pub fn resolve_port_owner(
    ports: &[OpenPortEntry],
    port: Port,
) -> Result<Resolution<Pid>, InspectError> {
    let mut owners: Vec<Pid> = Vec::new();
    let mut matched = false;
    for entry in ports.iter().filter(|e| e.port == port) {
        matched = true;
        if let Some(pid) = entry.pid {
            owners.push(pid);
        }
    }
    port_resolution(matched, owners, port)
}

/// 端口属主解析（Socket 条目输入，语义与 [`resolve_port_owner`] 一致）。
///
/// # Errors
/// 见 [`resolve_port_owner`]。
pub fn resolve_port_owner_in_sockets(
    sockets: &[SocketEntry],
    port: Port,
) -> Result<Resolution<Pid>, InspectError> {
    let mut owners: Vec<Pid> = Vec::new();
    let mut matched = false;
    for socket in sockets
        .iter()
        .filter(|s| s.protocol != Protocol::Unix && s.port == Some(port))
    {
        matched = true;
        if let Some(pid) = socket.owner_pid {
            owners.push(pid);
        }
    }
    port_resolution(matched, owners, port)
}

/// 端口属主集合 → 解析结果（无条目 `NotFound` / 有条目无属主
/// `SocketOwnerUnknown` / 去重升序 `Unique` 或 `Ambiguous`）。
fn port_resolution(
    matched: bool,
    owners: Vec<Pid>,
    port: Port,
) -> Result<Resolution<Pid>, InspectError> {
    if !matched {
        return Err(InspectError::NotFound {
            subject: format!("端口 {port} 的监听条目"),
        });
    }
    if owners.is_empty() {
        return Err(InspectError::SocketOwnerUnknown {
            subject: format!("端口 {port}"),
        });
    }
    Ok(from_sorted_pids(owners))
}

/// 容器匹配输入：平台清单条目 + 临时 Compose 键（core 不持有 Compose 富化
/// 字段，由调用方在采集时携带）。
#[derive(Debug, Clone, Copy)]
pub struct ContainerMatchInput<'a> {
    /// 容器清单条目。
    pub summary: &'a ContainerSummary,
    /// 容器主命令（运行时 inspect 结果；不可得为 `None`）。
    pub command: Option<&'a str>,
    /// Compose 项目名（非 Compose 容器为 `None`）。
    pub compose_project: Option<&'a str>,
    /// Compose 服务名（非 Compose 容器为 `None`）。
    pub compose_service: Option<&'a str>,
}

/// 容器解析（parity §2/§7 `ResolveContainer` / `matchContainer`）。
///
/// 匹配字段：name、image、command、Compose project、Compose service——
/// 大小写不敏感；exact 时要求与任一非空字段全等，否则子串包含；空字段跳过。
/// 结果按 runtime + id（[`ContainerKey`]）去重（不得按短 ID 合并），并按
/// `(runtime, id)` 升序稳定排序；无结果返回 [`InspectError::NotFound`]。
///
/// # Errors
/// 无命中时返回 [`InspectError::NotFound`]。
pub fn resolve_containers(
    candidates: &[ContainerMatchInput<'_>],
    query: &str,
    exact: bool,
) -> Result<Resolution<ContainerKey>, InspectError> {
    let lower = query.to_lowercase();
    let mut keys: Vec<ContainerKey> = Vec::new();
    for input in candidates {
        let summary = input.summary;
        let fields = [
            summary.name.as_deref().unwrap_or_default(),
            summary.image.as_deref().unwrap_or_default(),
            input.command.unwrap_or_default(),
            input.compose_project.unwrap_or_default(),
            input.compose_service.unwrap_or_default(),
        ];
        let matched = fields.iter().any(|field| {
            if field.is_empty() {
                return false;
            }
            let field = field.to_lowercase();
            if exact {
                field == lower
            } else {
                field.contains(&lower)
            }
        });
        if matched && !keys.contains(&summary.key) {
            keys.push(summary.key.clone());
        }
    }
    if keys.is_empty() {
        return Err(InspectError::NotFound {
            subject: format!("容器 {query:?}"),
        });
    }
    keys.sort();
    if keys.len() == 1 {
        Ok(Resolution::Unique(keys.remove(0)))
    } else {
        Ok(Resolution::Ambiguous(keys))
    }
}

/// 文件解析（parity §2 `ResolveFile`）：委托平台 `FileInventory.holders`
/// 结果做 Unique / Ambiguous 归并；空 → [`InspectError::NotFound`]（subject
/// 含路径，与 witr `no process found holding file: <path>` 对齐）。
///
/// # Errors
/// 无持有者时返回 [`InspectError::NotFound`]。
pub fn resolve_file_holders(
    holders: &[FileInventoryEntry],
    path: &Path,
) -> Result<Resolution<Pid>, InspectError> {
    let mut pids: Vec<Pid> = holders.iter().map(|entry| entry.pid).collect();
    if pids.is_empty() {
        return Err(InspectError::NotFound {
            subject: format!("持有文件 {} 的进程", path.display()),
        });
    }
    pids.sort_unstable();
    pids.dedup();
    Ok(from_sorted_pids(pids))
}

/// 候选集合 → [`Resolution`]（空 → NotFound，其余按已排序集合归并）。
fn resolution_from_pids(pids: Vec<Pid>, subject: String) -> Result<Resolution<Pid>, InspectError> {
    if pids.is_empty() {
        return Err(InspectError::NotFound { subject });
    }
    Ok(from_sorted_pids(pids))
}

/// 已排序的 PID 集合 → [`Resolution`]（单元素归并为 `Unique`）。
fn from_sorted_pids(mut pids: Vec<Pid>) -> Resolution<Pid> {
    pids.sort_unstable();
    pids.dedup();
    if pids.len() == 1 {
        Resolution::Unique(pids[0])
    } else {
        Resolution::Ambiguous(pids)
    }
}
