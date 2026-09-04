use super::*;
#[test]
fn linux_source_evidence_collects_raw_inputs_without_judging() -> TestResult {
    let temp = TempProc::new("evidence")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 1, "systemd")?;
    write_basic_process(&temp, 100, "fxt-app")?;
    temp.write("100/cgroup", "0::/system.slice/fxt-app.service\n")?;
    temp.write("1/cgroup", "0::/init.scope\n")?;
    temp.write("100/environ", "SNAP_NAME=fxt-snap\0FXT_SECRET=redacted\0")?;
    // systemd 运行探测目录不存在 → false。
    let platform = platform_for(&temp)?;

    let ancestry = ProcessInventory::list(&platform)
        .data
        .ok_or_else(|| String::from("基线采集不应整体失败"))?;
    // list 升序给出 [1, 100]（根 → 目标）。
    let evidence = SourceEvidenceProvider::evidence(&platform, &ancestry);
    assert!(!evidence.systemd_running);
    assert!(evidence.systemd_details.is_empty());
    assert_eq!(evidence.cgroup_by_pid.len(), 2);
    assert_eq!(evidence.cgroup_by_pid[0].0.get(), 1);
    assert!(evidence.cgroup_by_pid[0].1.contains("init.scope"));
    let env_of_100 = &evidence
        .env_by_pid
        .iter()
        .find(|(owner, _)| owner.get() == 100)
        .ok_or_else(|| String::from("缺少目标进程环境"))?
        .1;
    assert!(env_of_100.contains(&(String::from("SNAP_NAME"), String::from("fxt-snap"))));
    // 敏感性：值不进入任何诊断——本实现不产生诊断，仅确认键值对完整。
    Ok(())
}

#[test]
fn linux_source_evidence_systemd_probe_uses_injected_dir() -> TestResult {
    let temp = TempProc::new("systemd")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 100, "fxt-app")?;
    // systemd 运行目录存在 → systemd_running = true；单元不存在于总线时
    // 富化按 best-effort 省略（无总线/无单元 → details 为空）。
    fs::create_dir_all(temp.root.join("run/systemd/system"))?;
    let platform = platform_for(&temp)?;

    let ancestry = ProcessInventory::list(&platform)
        .data
        .ok_or_else(|| String::from("基线采集不应整体失败"))?;
    let evidence = SourceEvidenceProvider::evidence(&platform, &ancestry);
    assert!(evidence.systemd_running);
    // 合成 cgroup 含 fxt-app.service；真实系统总线没有该单元 → details 空。
    assert!(
        evidence.systemd_details.is_empty(),
        "best-effort 富化失败必须省略而不是伪造"
    );
    Ok(())
}

#[test]
fn linux_capability_status_is_supported_on_linux() -> TestResult {
    let temp = TempProc::new("capability")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    let platform = platform_for(&temp)?;
    assert_eq!(
        ProcessInventory::capability(&platform),
        runquiry_core::CapabilityStatus::Supported
    );
    assert_eq!(
        NetworkInventory::capability(&platform),
        runquiry_core::CapabilityStatus::Supported
    );
    assert_eq!(
        FileInventory::capability(&platform),
        runquiry_core::CapabilityStatus::Supported
    );
    Ok(())
}
