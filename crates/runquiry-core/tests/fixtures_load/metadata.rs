//! fixture 封套的共享元数据断言。

use std::time::{Duration, SystemTime};

use crate::support::fixtures::LoadedFixture;

/// 覆盖的两个合成平台目录名（macOS 已移出 v1 范围）。
pub const PLATFORMS: [&str; 2] = ["linux", "windows"];
/// 固定 fixture 采集时间的毫秒时间戳。
pub const CAPTURED_AT_MS: u64 = 1_700_000_000_000;

/// 返回固定 fixture 采集时间。
pub fn expected_captured_at() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_millis(CAPTURED_AT_MS)
}

/// 断言封套的平台、场景、generation 与采集时间。
pub fn assert_metadata<T>(fixture: &LoadedFixture<T>, platform: &str, scenario: &str) {
    assert_eq!(fixture.platform, platform, "平台名必须与目录一致");
    assert_eq!(fixture.scenario, scenario, "场景名必须与文件名对应");
    assert_eq!(
        fixture.generation,
        crate::support::Generation::FIXTURE.get()
    );
    assert_eq!(
        fixture.captured_at,
        expected_captured_at(),
        "采集时刻必须确定性"
    );
}
