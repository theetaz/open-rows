// SPDX-License-Identifier: Apache-2.0
//! Local history retention, privacy and isolation regressions.
use super::*;
#[test]
fn bucket_count_is_bounded_and_gaps_survive_downsampling() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = tempfile::tempdir()?;
    let store = LocalStore::open(&dir.path().join("test.sqlite"))?;
    store.set_monitor_profiles(&["a".into()], 600)?;
    for start in (1..=600).step_by(60) {
        let points: Vec<_> = (start..start + 60).map(point).collect();
        store.record_monitor(("a", "db"), &points, 600)?;
    }
    let mut gap = point(0);
    gap.state = "gap".into();
    gap.value = None;
    store.record_monitor(("a", "db"), &[gap], 600)?;
    let points = store.monitor_history(("a", "db"), (0, 600))?;
    assert!(points.len() <= 300);
    assert_eq!(points[0].state, "gap");
    assert_eq!(points[0].value, None);
    Ok(())
}
fn point(time: i64) -> MonitorAggregate {
    MonitorAggregate {
        metric: "Database:xact_commit".into(),
        collected_ms: time,
        value: Some(1.0),
        state: "ok".into(),
    }
}
#[test]
fn default_off_enable_disable_purge_expiry_and_profile_isolation()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let store = LocalStore::open(&dir.path().join("test.sqlite"))?;
    store.record_monitor(("a", "db"), &[point(1)], 1)?;
    assert!(store.monitor_history(("a", "db"), (0, 1))?.is_empty());
    store.set_monitor_profiles(&["a".into(), "b".into()], 1)?;
    for scope in [("a", "db"), ("b", "db"), ("a", "other")] {
        store.record_monitor(scope, &[point(1)], 1)?;
    }
    assert_eq!(store.monitor_history(("a", "db"), (0, 1))?.len(), 1);
    store.purge_monitor_history(Some("a"))?;
    assert_eq!(store.monitor_history(("b", "db"), (0, 1))?.len(), 1);
    assert!(store.monitor_history(("a", "other"), (0, 1))?.is_empty());
    store.set_monitor_profiles(&["a".into()], 1)?;
    assert!(store.monitor_history(("b", "db"), (0, 1))?.is_empty());
    store.record_monitor(("a", "db"), &[point(1)], 1)?;
    assert!(
        store
            .monitor_history(("a", "db"), (0, DAY_MS + 2))?
            .is_empty()
    );
    store.record_monitor(("a", "db"), &[point(DAY_MS + 2)], DAY_MS + 2)?;
    store.purge_monitor_history(None)?;
    assert!(
        store
            .monitor_history(("a", "db"), (0, DAY_MS + 2))?
            .is_empty()
    );
    Ok(())
}
#[test]
fn startup_expiry_removes_old_history() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("test.sqlite");
    let store = LocalStore::open(&path)?;
    store.set_monitor_profiles(&["a".into()], 10)?;
    store.record_monitor(("a", "db"), &[point(1)], 10)?;
    drop(store);
    let store = LocalStore::open(&path)?;
    assert!(store.monitor_history(("a", "db"), (0, 10))?.is_empty());
    Ok(())
}
#[test]
fn fixed_byte_caps_evict_oldest_and_raw_text_is_rejected() -> Result<(), Box<dyn std::error::Error>>
{
    assert_eq!(
        (CONNECTION_ROWS * 2_048, APP_ROWS * 2_048),
        (50 * 1_024 * 1_024, 100 * 1_024 * 1_024)
    );
    let dir = tempfile::tempdir()?;
    let store = LocalStore::open(&dir.path().join("test.sqlite"))?;
    store.set_monitor_profiles(&["a".into(), "b".into(), "c".into()], 10)?;
    for profile in ["a", "b", "c"] {
        for time in 1..=4 {
            store.record_monitor((profile, "db"), &[point(time)], 10)?;
        }
    }
    enforce(&*store.lock()?, 10, 2, 5)?;
    assert_eq!(store.monitor_history(("a", "db"), (0, 10))?.len(), 2);
    assert_eq!(store.monitor_history(("c", "db"), (0, 10))?.len(), 1);
    let mut unsafe_point = point(10);
    unsafe_point.metric = "SELECT 'test-only'".into();
    assert!(
        store
            .record_monitor(("a", "db"), &[unsafe_point], 10)
            .is_err()
    );
    Ok(())
}

#[test]
fn mysql_server_history_allows_only_reviewed_scalars() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let store = LocalStore::open(&dir.path().join("test.sqlite"))?;
    store.set_monitor_profiles(&["a".into()], 1)?;
    let points: Vec<_> = [
        "queries",
        "threads_connected",
        "threads_running",
        "buffer_pool_hit_ratio",
        "rows_read",
        "rows_inserted",
        "rows_updated",
        "rows_deleted",
        "temp_disk_tables",
        "slow_queries",
        "aborted_connects",
        "deadlocks",
    ]
    .map(|name| MonitorAggregate {
        metric: format!("Database:{name}"),
        ..point(1)
    })
    .into();
    store.record_monitor(("a", "db"), &points, 1)?;
    assert_eq!(
        store.monitor_history(("a", "db"), (0, 1))?.len(),
        points.len()
    );
    let raw = MonitorAggregate {
        metric: "Database:digest_text".into(),
        ..point(1)
    };
    assert!(store.record_monitor(("a", "db"), &[raw], 1).is_err());
    Ok(())
}
