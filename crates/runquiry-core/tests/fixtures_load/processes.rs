//! 进程快照 fixture 的正常、空、部分成功与失败语义。

use runquiry_core::ProcessSummary;

use crate::support::fixtures::load;

use super::metadata::{PLATFORMS, assert_metadata};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn processes_normal_fixture_loads_for_all_platforms() -> TestResult {
    for platform in PLATFORMS {
        let fixture = load::<Vec<ProcessSummary>>(&format!("{platform}/processes-normal.json"))?;
        assert_metadata(&fixture, platform, "normal");
        let entries = fixture
            .inspection
            .data
            .as_deref()
            .ok_or_else(|| String::from("normal 场景必须携带数据"))?;
        assert!(!entries.is_empty());
        assert!(fixture.inspection.issues.is_empty());
        for entry in entries {
            assert!(entry.command.starts_with("fxt-"));
            assert!(entry.identity.pid().get() > 0);
        }
    }
    Ok(())
}

#[test]
fn processes_empty_fixture_yields_complete_empty_data() -> TestResult {
    for platform in PLATFORMS {
        let fixture = load::<Vec<ProcessSummary>>(&format!("{platform}/processes-empty.json"))?;
        assert_metadata(&fixture, platform, "empty");
        assert_eq!(
            fixture
                .inspection
                .data
                .as_deref()
                .map(<[ProcessSummary]>::len),
            Some(0)
        );
        assert!(fixture.inspection.issues.is_empty());
        assert!(!fixture.inspection.is_empty());
    }
    Ok(())
}

#[test]
fn processes_partial_fixture_keeps_data_alongside_permission_issue() -> TestResult {
    for platform in PLATFORMS {
        let fixture = load::<Vec<ProcessSummary>>(&format!("{platform}/processes-partial.json"))?;
        assert_metadata(&fixture, platform, "partial");
        let entries = fixture
            .inspection
            .data
            .as_deref()
            .ok_or_else(|| String::from("partial 场景必须保留已取得数据"))?;
        assert_eq!(entries.len(), 1);
        assert_eq!(fixture.inspection.issues.len(), 1);
        assert_eq!(
            fixture.inspection.issues[0].code().code(),
            "permission_denied"
        );
        assert!(fixture.inspection.has_issues());
    }
    Ok(())
}

#[test]
fn processes_permission_fixture_fails_without_data() -> TestResult {
    for platform in PLATFORMS {
        let fixture =
            load::<Vec<ProcessSummary>>(&format!("{platform}/processes-permission.json"))?;
        assert_metadata(&fixture, platform, "permission");
        assert!(fixture.inspection.is_empty());
        assert_eq!(
            fixture.inspection.issues[0].code().code(),
            "permission_denied"
        );
    }
    Ok(())
}
