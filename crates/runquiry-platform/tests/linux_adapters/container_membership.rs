use super::*;

#[test]
fn linux_container_membership_reads_the_candidate_pid_cgroup() -> TestResult {
    let temp = TempProc::new("container-membership")?;
    let container_id = "5f2d4a1b9c8e0000000000000000000000000000000000000000000000000aa1";
    temp.write(
        "4242/cgroup",
        &format!("0::/system.slice/docker-{container_id}.scope\n"),
    )?;
    temp.write("4343/cgroup", "0::/user.slice/user-1000.slice\n")?;
    let platform = platform_for(&temp)?;
    let key = ContainerKey {
        runtime: String::from("docker"),
        id: String::from(container_id),
    };

    assert!(platform.belongs_to_container(Pid::new(4_242)?, &key));
    assert!(!platform.belongs_to_container(Pid::new(4_343)?, &key));
    assert!(!platform.belongs_to_container(Pid::new(4_444)?, &key));
    Ok(())
}
