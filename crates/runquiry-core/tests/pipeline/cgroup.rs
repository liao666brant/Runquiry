//! cgroup 与 systemd 解析契约。

use runquiry_core::{
    detect_container_from_cgroup, detect_lxc_runtime, find_long_hex_id, short_id,
    systemd_unit_from_cgroup,
};

use super::{LONG_HEX, TestResult};

#[test]
fn pipeline_cgroup_docker_scope_and_path_patterns_yield_container_context() -> TestResult {
    // cgroup v2 scope 模式：docker-<64hex>.scope。
    let v2 = format!("0::/system.slice/docker-{LONG_HEX}.scope\n");
    let ctx = detect_container_from_cgroup(&v2)
        .ok_or_else(|| String::from("docker scope 应识别出容器上下文"))?;
    assert_eq!(ctx.runtime(), "docker");
    assert_eq!(ctx.container_id(), LONG_HEX);

    // cgroup v1 路径模式：/docker/<64hex>。
    let v1 = format!("1:name=systemd:/docker/{LONG_HEX}\n");
    let ctx = detect_container_from_cgroup(&v1)
        .ok_or_else(|| String::from("docker 路径应识别出容器上下文"))?;
    assert_eq!(ctx.runtime(), "docker");
    assert_eq!(ctx.container_id(), LONG_HEX);
    Ok(())
}

#[test]
fn pipeline_cgroup_podman_and_libpod_share_podman_runtime() -> TestResult {
    let scope = format!("0::/user.slice/user-1000.slice/libpod-{LONG_HEX}.scope\n");
    let ctx = detect_container_from_cgroup(&scope)
        .ok_or_else(|| String::from("libpod scope 应识别出容器上下文"))?;
    assert_eq!(ctx.runtime(), "podman");
    assert_eq!(ctx.container_id(), LONG_HEX);

    let path = format!("11:pids:/libpod/{LONG_HEX}\n");
    let ctx = detect_container_from_cgroup(&path)
        .ok_or_else(|| String::from("libpod 路径应识别出容器上下文"))?;
    assert_eq!(ctx.runtime(), "podman");
    assert_eq!(ctx.container_id(), LONG_HEX);
    Ok(())
}

#[test]
fn pipeline_cgroup_kubepods_uses_crictl_runtime_and_long_hex_id() -> TestResult {
    let content = format!("0::/kubepods.slice/kubepods-besteffort.slice/{LONG_HEX}\n");
    let ctx = detect_container_from_cgroup(&content)
        .ok_or_else(|| String::from("kubepods 应识别出容器上下文"))?;
    assert_eq!(ctx.runtime(), "crictl");
    assert_eq!(ctx.container_id(), LONG_HEX);
    Ok(())
}

#[test]
fn pipeline_cgroup_containerd_uses_nerdctl_runtime_and_long_hex_id() -> TestResult {
    let content = format!("0::/containerd/fxt/{LONG_HEX}\n");
    let ctx = detect_container_from_cgroup(&content)
        .ok_or_else(|| String::from("containerd 应识别出容器上下文"))?;
    assert_eq!(ctx.runtime(), "nerdctl");
    assert_eq!(ctx.container_id(), LONG_HEX);
    Ok(())
}

#[test]
fn pipeline_cgroup_colima_yields_scope_id_or_default_without_id() -> TestResult {
    let with_id = "0::/user.slice/colima-cafe1234.scope\n";
    let ctx = detect_container_from_cgroup(with_id)
        .ok_or_else(|| String::from("colima scope 应识别出容器上下文"))?;
    assert_eq!(ctx.runtime(), "colima");
    assert_eq!(ctx.container_id(), "cafe1234");

    // witr 对无 ID 的 colima 记 "colima: default"：上下文存在但无 ID。
    let default = "0::/colima/default\n";
    let ctx = detect_container_from_cgroup(default)
        .ok_or_else(|| String::from("colima 默认实例应识别出容器上下文"))?;
    assert_eq!(ctx.runtime(), "colima");
    assert_eq!(ctx.container_id(), "");
    Ok(())
}

#[test]
fn pipeline_cgroup_lxc_payload_yields_lxc_runtime_with_payload_name() -> TestResult {
    let content = "0::/lxc.payload.fxt-container/user/0\n";
    let ctx = detect_container_from_cgroup(content)
        .ok_or_else(|| String::from("lxc.payload 应识别出容器上下文"))?;
    assert_eq!(ctx.runtime(), "lxc");
    assert_eq!(ctx.container_id(), "fxt-container");
    Ok(())
}

#[test]
fn pipeline_cgroup_systemd_service_is_not_a_container() {
    assert_eq!(
        detect_container_from_cgroup("0::/system.slice/nginx.service\n"),
        None,
        "systemd 单元路径不得误判为容器"
    );
    assert_eq!(detect_container_from_cgroup(""), None);
    assert_eq!(
        detect_container_from_cgroup("0::/user.slice/user-1000.slice\n"),
        None
    );
}

#[test]
fn pipeline_long_hex_id_extraction_follows_witr_semantics() {
    assert_eq!(
        find_long_hex_id(&format!("prefix-{LONG_HEX}-suffix")),
        Some(LONG_HEX.to_string())
    );
    // 大写十六进制同样命中（witr 逐字符校验 0-9a-fA-F）。
    let upper = LONG_HEX.to_uppercase();
    assert_eq!(find_long_hex_id(&upper), Some(upper.clone()));
    // 不足 64 位不命中。
    assert_eq!(find_long_hex_id("0123456789abcdef"), None);
    // 超长十六进制串取首个 64 字符窗口。
    let long65 = format!("{LONG_HEX}f");
    assert_eq!(find_long_hex_id(&long65), Some(LONG_HEX.to_string()));
    // 中途混入非十六进制字符打断窗口。
    let broken = format!("{LONG_HEX}g{LONG_HEX}");
    assert_eq!(find_long_hex_id(&broken), Some(LONG_HEX.to_string()));
    assert_eq!(find_long_hex_id(""), None);
}

#[test]
fn pipeline_short_id_truncates_to_twelve_characters() {
    assert_eq!(short_id(LONG_HEX), &LONG_HEX[..12]);
    assert_eq!(short_id("short-id"), "short-id");
    assert_eq!(short_id("exactly12chr"), "exactly12chr");
}

#[test]
fn pipeline_lxc_runtime_detection_matches_witr_ancestor_commands() {
    // witr `TestDetectLXCRuntime`：命令名精确匹配（incusd 而非 incus）。
    assert_eq!(detect_lxc_runtime("incusd"), "incus");
    assert_eq!(detect_lxc_runtime("lxd"), "lxd");
    assert_eq!(detect_lxc_runtime("lxc-start"), "lxc");
    assert_eq!(detect_lxc_runtime("unrelated"), "lxc");
    assert_eq!(detect_lxc_runtime(""), "lxc");
}

#[test]
fn pipeline_systemd_unit_from_cgroup_parses_v1_v2_and_scope() {
    // cgroup v2：controllers 为空。
    assert_eq!(
        systemd_unit_from_cgroup("0::/system.slice/nginx.service\n"),
        Some(String::from("nginx.service"))
    );
    // cgroup v1：controller 含 name=systemd。
    assert_eq!(
        systemd_unit_from_cgroup("1:name=systemd:/system.slice/nginx.service\n"),
        Some(String::from("nginx.service"))
    );
    // 非 systemd controller 的 v1 行被跳过。
    assert_eq!(
        systemd_unit_from_cgroup("11:pids:/system.slice/nginx.service\n"),
        None
    );
    // witr 同样接受 .scope 结尾（用户会话作用域）。
    assert_eq!(
        systemd_unit_from_cgroup(
            "0::/user.slice/user-1000.slice/user@1000.service/app.slice/app-fxt.scope\n"
        ),
        Some(String::from("app-fxt.scope"))
    );
    // 嵌套路径取最深命中单元。
    assert_eq!(
        systemd_unit_from_cgroup("0::/system.slice/foo.service/subgroup\n"),
        Some(String::from("foo.service"))
    );
    // 无单元命中时返回 None。
    assert_eq!(systemd_unit_from_cgroup("0::/\n"), None);
    assert_eq!(systemd_unit_from_cgroup(""), None);
}
