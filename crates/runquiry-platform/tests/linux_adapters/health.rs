use super::*;
/// 健康标签：累计 CPU > 2h → HighCpu（healthy 时判定、优先于 high-mem），
/// RSS > 1GiB → HighMem；阈值边界（恰好 7200s / 1GiB）保持 Healthy
/// （parity witr process_linux.go:193-200）。
#[test]
fn linux_health_labels_compute_high_cpu_and_high_memory() -> TestResult {
    let temp = TempProc::new("health-labels")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    // 700000+700000 ticks = 14000s > 7200s。
    temp.write(
        "100/stat",
        &stat_content_full(
            100,
            "fxt-hot",
            'S',
            1,
            100,
            StatHot {
                utime: 700_000,
                stime: 700_000,
                ..StatHot::default()
            },
        ),
    )?;
    // RSS 262145 页 × 4K = 1073758208 > 1GiB。
    temp.write(
        "101/stat",
        &stat_content_full(
            101,
            "fxt-fat",
            'S',
            1,
            100,
            StatHot {
                rss_pages: 262_145,
                ..StatHot::default()
            },
        ),
    )?;
    // 边界：7200s 整、1GiB 整 → 仍 Healthy。
    temp.write(
        "102/stat",
        &stat_content_full(
            102,
            "fxt-edge",
            'S',
            1,
            100,
            StatHot {
                utime: 719_995,
                rss_pages: 262_144,
                ..StatHot::default()
            },
        ),
    )?;
    for pid in [100_u32, 101, 102] {
        temp.write(&format!("{pid}/status"), &status_content("fxt", 54321, "0"))?;
        temp.write(&format!("{pid}/comm"), "fxt\n")?;
    }
    let platform = platform_for(&temp)?;
    let listed = ProcessInventory::list(&platform);
    let summaries = listed.data.ok_or("应有基线数据")?;
    let health_of = |pid: u32| -> Result<HealthStatus, String> {
        summaries
            .iter()
            .find(|entry| entry.identity.pid().get() == pid)
            .map(|entry| entry.health)
            .ok_or_else(|| String::from("进程应在基线中"))
    };
    assert_eq!(health_of(100)?, HealthStatus::HighCpu);
    assert_eq!(health_of(101)?, HealthStatus::HighMem);
    assert_eq!(health_of(102)?, HealthStatus::Healthy);
    Ok(())
}

/// 自身排除的时间窗兜底：构造时刻之后启动、且父进程已退出（PPID 链断裂，
/// 被 1 号收养）的进程同样被排除（修复前 PPID 链查不到即保留）。
#[test]
fn linux_late_reparented_helper_is_excluded_by_time_window() -> TestResult {
    let temp = TempProc::new("late-reparented")?;
    temp.write("stat", &format!("btime {BOOT_TIME_SECS}\n"))?;
    write_basic_process(&temp, 1, "systemd")?;
    // 构造时刻：btime + 2s。PID 500 启动于 btime + 5s（晚于构造），父为 1。
    let construction = SystemTime::UNIX_EPOCH + Duration::from_secs(BOOT_TIME_SECS + 2);
    let platform = platform_for(&temp)?.with_constructed_at(construction);
    temp.write("500/stat", &stat_content(500, "fxt-helper", 'S', 1, 500))?;
    temp.write("500/status", &status_content("fxt-helper", 54321, "0"))?;
    temp.write("500/comm", "fxt-helper\n")?;
    let listed = ProcessInventory::list(&platform);
    let summaries = listed.data.ok_or("应有基线数据")?;
    assert!(
        !summaries
            .iter()
            .any(|entry| entry.identity.pid().get() == 500),
        "启动晚于构造时刻的收养后代应被时间窗排除"
    );
    // 构造时刻之前启动的普通进程不受影响。
    assert!(
        summaries
            .iter()
            .any(|entry| entry.identity.pid().get() == 1),
        "既有进程不应被时间窗误排除"
    );
    Ok(())
}
