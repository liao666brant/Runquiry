#![cfg(unix)]
//! Incus / LXD（lxd-like 家族）测试：共享 REST JSON 解析、`list <name>` 的
//! state.pid 主机 PID、LXD 双二进制探测约束与跨运行时不合并。

#[path = "container_support.rs"]
mod support;

use runquiry_core::{ContainerKey, Pid};
use runquiry_platform::container::{ContainerRuntimes, RuntimeBinaries};
use support::{TempDir, TestResult, absent_binaries, fake_cli, subcommand_response};

fn incus_key(id: &str) -> ContainerKey {
    ContainerKey {
        runtime: String::from("incus"),
        id: String::from(id),
    }
}

fn lxd_key(id: &str) -> ContainerKey {
    ContainerKey {
        runtime: String::from("lxd"),
        id: String::from(id),
    }
}

const INSTANCES_JSON: &str = r#"[
  {"name":"c1","type":"container","status":"Running","created_at":"2026-08-30T10:00:00Z",
   "config":{"image.description":"Debian 13"}, "state":{"status":"Running","pid":5101}},
  {"name":"c2","type":"container","status":"Stopped","created_at":"2026-08-30T11:00:00Z",
   "config":{"image.os":"Alpine","image.release":"3.20"}, "state":{"status":"Stopped","pid":0}}
]"#;

#[test]
fn container_incus_list_parses_shared_rest_shape() -> TestResult {
    let dir = TempDir::new("incus-list")?;
    let incus = fake_cli(&dir, "incus", &subcommand_response("list", INSTANCES_JSON))?;
    let inventory = ContainerRuntimes::with_binaries(RuntimeBinaries {
        incus: incus.display().to_string(),
        ..absent_binaries()
    });
    let items = inventory.list_detailed().data.ok_or("应有 incus 数据")?;
    assert_eq!(items.len(), 2);
    let c1 = &items[0];
    assert_eq!(c1.summary.key.runtime, "incus");
    assert_eq!(c1.summary.key.id, "c1");
    assert_eq!(c1.summary.name.as_deref(), Some("c1"));
    assert_eq!(c1.summary.image.as_deref(), Some("Debian 13"));
    assert_eq!(c1.summary.status.as_deref(), Some("Running"));
    // 列表载荷自带 state.pid：运行中实例的主机 PID 直接可用。
    assert_eq!(c1.summary.host_pid, Some(Pid::new(5101)?));
    // 镜像回退到 image.os + image.release；停止实例 PID 为 0 → None。
    assert_eq!(items[1].summary.image.as_deref(), Some("Alpine 3.20"));
    assert_eq!(items[1].summary.host_pid, None);
    Ok(())
}

#[test]
fn container_lxd_requires_client_and_daemon_binaries() -> TestResult {
    let dir = TempDir::new("lxd-probe")?;
    // 只有客户端 `lxc`、没有守护进程 `lxd`：不得把经典 LXC 工具链误判为 LXD。
    let client = fake_cli(&dir, "lxc", "")?;
    let inventory = ContainerRuntimes::with_binaries(RuntimeBinaries {
        lxd_client: client.display().to_string(),
        ..absent_binaries()
    });
    let items = inventory.list_detailed().data;
    assert_eq!(items, None, "缺 lxd 守护进程时 LXD 不可用");
    Ok(())
}

#[test]
fn container_lxd_host_pid_uses_list_by_name() -> TestResult {
    let dir = TempDir::new("lxd-hostpid")?;
    let client = fake_cli(&dir, "lxc", &subcommand_response("list", INSTANCES_JSON))?;
    let daemon = fake_cli(&dir, "lxd", "")?;
    let inventory = ContainerRuntimes::with_binaries(RuntimeBinaries {
        lxd_client: client.display().to_string(),
        lxd_daemon: daemon.display().to_string(),
        ..absent_binaries()
    });
    assert_eq!(inventory.host_pid(&lxd_key("c1"))?, Some(Pid::new(5101)?));
    // 名称未精确命中时回退首条（parity：lxdLikeHostPID）。
    assert_eq!(
        inventory.host_pid(&lxd_key("missing"))?,
        Some(Pid::new(5101)?)
    );
    // 富集为 no-op（网络/挂载不在本轮范围）。
    assert_eq!(inventory.enrich(&lxd_key("c1"))?.started_at, None);
    Ok(())
}

#[test]
fn container_same_instance_name_across_incus_and_lxd_is_not_merged() -> TestResult {
    let dir = TempDir::new("lxdlike-dedup")?;
    let incus = fake_cli(&dir, "incus", &subcommand_response("list", INSTANCES_JSON))?;
    let client = fake_cli(&dir, "lxc", &subcommand_response("list", INSTANCES_JSON))?;
    let daemon = fake_cli(&dir, "lxd", "")?;
    let inventory = ContainerRuntimes::with_binaries(RuntimeBinaries {
        incus: incus.display().to_string(),
        lxd_client: client.display().to_string(),
        lxd_daemon: daemon.display().to_string(),
        ..absent_binaries()
    });
    let items = inventory.list_detailed().data.ok_or("应有数据")?;
    // 同名实例在 incus 与 lxd 下是两个容器（runtime + id 去重键）。
    assert!(
        items
            .iter()
            .any(|entry| entry.summary.key == incus_key("c1"))
    );
    assert!(items.iter().any(|entry| entry.summary.key == lxd_key("c1")));
    assert_eq!(items.len(), 4);
    Ok(())
}
