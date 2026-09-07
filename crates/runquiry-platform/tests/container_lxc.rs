//! 经典 LXC 测试：`lxc-ls --fancy --format json` 列表解析与
//! `lxc-info -n <name> -p -H` 主机 PID。
#![cfg(unix)]

#[path = "container_support.rs"]
mod support;

use runquiry_core::{ContainerInventory, ContainerKey, DiagnosticCode, Pid};
use runquiry_platform::container::{ContainerRuntimes, RuntimeBinaries};
use support::{TempDir, TestResult, absent_binaries, fake_cli, subcommand_response};

fn lxc_inventory(lxc_ls: &std::path::Path, lxc_info: &std::path::Path) -> ContainerRuntimes {
    ContainerRuntimes::with_binaries(RuntimeBinaries {
        lxc_ls: lxc_ls.display().to_string(),
        lxc_info: lxc_info.display().to_string(),
        ..absent_binaries()
    })
}

fn lxc_key(id: &str) -> ContainerKey {
    ContainerKey {
        runtime: String::from("lxc"),
        id: String::from(id),
    }
}

#[test]
fn container_lxc_list_parses_fancy_json() -> TestResult {
    let dir = TempDir::new("lxc-list")?;
    let body = subcommand_response(
        "--fancy",
        r#"[{"name":"web","state":"RUNNING","autostart":"1","groups":"-","ipv4":"10.0.0.2","ipv6":"-","unprivileged":"true"},
            {"name":"db","state":"STOPPED","autostart":"0","groups":"-","ipv4":"-","ipv6":"-","unprivileged":"false"}]"#,
    );
    let lxc_ls = fake_cli(&dir, "lxc-ls", &body)?;
    let lxc_info = fake_cli(&dir, "lxc-info", "")?;
    let inventory = lxc_inventory(&lxc_ls, &lxc_info);
    let items = ContainerInventory::list(&inventory)
        .data
        .ok_or("应有 lxc 数据")?;
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].key.runtime, "lxc");
    assert_eq!(items[0].key.id, "web");
    assert_eq!(items[0].name.as_deref(), Some("web"));
    assert_eq!(items[0].status.as_deref(), Some("RUNNING"));
    assert_eq!(items[0].image, None, "经典 LXC 无镜像元数据");
    assert_eq!(items[0].host_pid, None, "PID 由 host_pid 惰性补齐");
    Ok(())
}

#[test]
fn container_lxc_host_pid_from_lxc_info() -> TestResult {
    let dir = TempDir::new("lxc-hostpid")?;
    let lxc_ls = fake_cli(&dir, "lxc-ls", &subcommand_response("--fancy", "[]"))?;
    let lxc_info = fake_cli(
        &dir,
        "lxc-info",
        // 存在但未运行的容器：lxc-info 正常退出且无 PID 输出。
        "if [ \"$2\" = \"web\" ]; then\n  echo 4321\nfi\nexit 0\n",
    )?;
    let inventory = lxc_inventory(&lxc_ls, &lxc_info);
    assert_eq!(inventory.host_pid(&lxc_key("web"))?, Some(Pid::new(4321)?));
    // 停止容器：lxc-info 输出为空或非正数 → None，不伪造。
    assert_eq!(inventory.host_pid(&lxc_key("stopped"))?, None);
    assert_eq!(inventory.enrich(&lxc_key("web"))?.started_at, None);
    Ok(())
}

#[test]
fn container_lxc_corrupt_list_json_is_parse_failed() -> TestResult {
    let dir = TempDir::new("lxc-corrupt")?;
    let lxc_ls = fake_cli(&dir, "lxc-ls", &subcommand_response("--fancy", "[{bad}]"))?;
    let lxc_info = fake_cli(&dir, "lxc-info", "")?;
    let inventory = lxc_inventory(&lxc_ls, &lxc_info);
    let inspection = ContainerInventory::list(&inventory);
    assert_eq!(inspection.data, None);
    assert!(inspection.issues.iter().any(
        |issue| issue.code() == DiagnosticCode::ParseFailed && issue.message().contains("lxc")
    ));
    Ok(())
}
