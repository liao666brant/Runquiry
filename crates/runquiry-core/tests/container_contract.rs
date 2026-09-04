//! 容器边界契约：containerd 稳定键与主机 PID 的 cgroup 二次校验。

use runquiry_core::{ContainerKey, Pid, verified_host_pid};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn host_pid_requires_matching_cgroup_runtime_and_id() -> TestResult {
    let pid = Some(Pid::new(4242)?);
    let id = "5f2d4a1b9c8e0000000000000000000000000000000000000000000000000aa1";
    let key = ContainerKey {
        runtime: String::from("docker"),
        id: id.to_string(),
    };
    let matching = format!("0::/system.slice/docker-{id}.scope\n");

    assert_eq!(verified_host_pid(pid, &key, Some(&matching)), pid);
    assert_eq!(verified_host_pid(pid, &key, None), None);
    assert_eq!(verified_host_pid(pid, &key, Some("0::/user.slice\n")), None);
    Ok(())
}

#[test]
fn host_pid_rejects_stale_pid_from_another_container() -> TestResult {
    let pid = Some(Pid::new(4242)?);
    let id = "5f2d4a1b9c8e0000000000000000000000000000000000000000000000000aa1";
    let other = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let key = ContainerKey {
        runtime: String::from("docker"),
        id: id.to_string(),
    };
    let cgroup = format!("0::/system.slice/docker-{other}.scope\n");

    assert_eq!(verified_host_pid(pid, &key, Some(&cgroup)), None);
    Ok(())
}

#[test]
fn host_pid_accepts_containerd_key_without_comparing_context_runtime_name() -> TestResult {
    let pid = Some(Pid::new(4242)?);
    let id = "abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd";
    let key = ContainerKey {
        runtime: String::from("containerd"),
        id: id.to_string(),
    };
    let cgroup = format!("0::/containerd/io.containerd.runtime.v2.task/default/{id}\n");

    assert_eq!(verified_host_pid(pid, &key, Some(&cgroup)), pid);
    Ok(())
}
