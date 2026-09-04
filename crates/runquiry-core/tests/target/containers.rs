//! 按容器字段解析的契约。

use runquiry_core::{
    ContainerKey, ContainerMatchInput, ContainerSummary, Resolution, resolve_containers,
};

use super::TestResult;
fn container(name: Option<&str>, runtime: &str, id: &str, image: Option<&str>) -> ContainerSummary {
    ContainerSummary {
        key: ContainerKey {
            runtime: String::from(runtime),
            id: String::from(id),
        },
        name: name.map(String::from),
        image: image.map(String::from),
        status: None,
        health: None,
        host_pid: None,
        started_at: None,
    }
}

/// 容器解析：五字段匹配（name/image/command/compose project/service）、
/// exact 全等 / fuzzy 子串、空字段跳过、大小写在 API 内部统一。
#[test]
fn target_container_resolution_matches_five_fields_with_exact_and_fuzzy() -> TestResult {
    let summary = container(Some("fxt-web"), "docker", "abc123", Some("nginx:latest"));
    let key = summary.key.clone();
    let inputs = vec![ContainerMatchInput {
        summary: &summary,
        command: Some("nginx -g daemon off"),
        compose_project: Some("fxt-stack"),
        compose_service: Some("web"),
    }];
    // fuzzy：name / compose service 子串（大小写不敏感）。
    assert_eq!(
        resolve_containers(&inputs, "fxt-web", false)?,
        Resolution::Unique(key.clone())
    );
    assert_eq!(
        resolve_containers(&inputs, "WEB", false)?,
        Resolution::Unique(key.clone())
    );
    // exact：字段全等命中（大小写不敏感）。
    assert_eq!(
        resolve_containers(&inputs, "NGINX:LATEST", true)?,
        Resolution::Unique(key)
    );
    // exact：非完整字段不命中（"fxt" ≠ "fxt-web"）→ NotFound。
    let err = resolve_containers(&inputs, "fxt", true)
        .err()
        .ok_or_else(|| String::from("exact 部分串不得命中"))?;
    assert_eq!(err.code(), "not_found");
    // 无命中 → NotFound。
    let err = resolve_containers(&inputs, "postgres", false)
        .err()
        .ok_or_else(|| String::from("无命中应为 not_found"))?;
    assert_eq!(err.code(), "not_found");
    Ok(())
}

/// 容器解析：按 runtime+id 去重（不得按短 ID 合并）、稳定排序、多结果完整候选。
#[test]
fn target_container_resolution_dedups_by_runtime_and_id_and_sorts() -> TestResult {
    let a = container(Some("fxt-a"), "docker", "aaa111", None);
    let b = container(Some("fxt-b"), "docker", "aaa111999999", None);
    let c = container(Some("fxt-c"), "podman", "bbb222", None);
    let mut inputs = Vec::new();
    for s in [&a, &a, &b, &c] {
        inputs.push(ContainerMatchInput {
            summary: s,
            command: None,
            compose_project: None,
            compose_service: None,
        });
    }
    let resolved = resolve_containers(&inputs, "fxt-", false)?;
    let Resolution::Ambiguous(keys) = &resolved else {
        return Err(String::from("多容器命中应返回 Ambiguous").into());
    };
    // docker|aaa111 与 docker|aaa111999999 是不同键（不得按短 ID 合并），加 podman|bbb222。
    assert_eq!(keys.len(), 3, "runtime|id 去重后三个键全部保留");
    assert_eq!(
        keys[0],
        ContainerKey {
            runtime: String::from("docker"),
            id: String::from("aaa111")
        }
    );
    assert_eq!(
        keys[2],
        ContainerKey {
            runtime: String::from("podman"),
            id: String::from("bbb222")
        }
    );
    Ok(())
}
