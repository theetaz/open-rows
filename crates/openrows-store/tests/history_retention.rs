// SPDX-License-Identifier: Apache-2.0
//! Retention is transactional on writes and sweeps rows already on disk.
use openrows_store::{HistoryRecord, HistoryRetention, HistorySource, HistoryStatus, LocalStore};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |time| {
            i64::try_from(time.as_millis()).unwrap_or(i64::MAX)
        })
}
fn record(profile: &str, started: i64) -> HistoryRecord {
    HistoryRecord {
        profile_id: profile.into(),
        database: Some("shop".into()),
        started_at_ms: started,
        duration_ms: None,
        status: HistoryStatus::Ok,
        rows: None,
        statement: "SELECT x".into(),
        error_message: None,
        source: HistorySource::Editor,
    }
}
#[test]
fn count_and_age_are_enforced_on_write_per_connection() -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = LocalStore::open(&dir.path().join("history.sqlite"))?;
    let now = now_ms();
    store.set_history_retention(
        HistoryRetention {
            max_entries: 2,
            max_age_days: 1,
            keep_forever: false,
        },
        now,
    )?;
    store.record_history(&record("a", now - 2 * 86_400_000))?;
    assert!(store.search_history(Some("a"), "", 10)?.is_empty());
    for started in [now - 3, now - 2, now - 1] {
        store.record_history(&record("a", started))?;
    }
    store.record_history(&record("b", now))?;
    let entries = store.search_history(Some("a"), "", 10)?;
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[1].record.started_at_ms, now - 2);
    assert_eq!(store.search_history(Some("b"), "", 10)?.len(), 1);
    assert_eq!(store.search_history(Some("a"), "SELECT", 10)?.len(), 2);
    Ok(())
}
#[test]
fn startup_sweeps_existing_rows_and_keep_forever_bypasses_limits() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("history.sqlite");
    let now = now_ms();
    let store = LocalStore::open(&path)?;
    for started in [now - 3 * 86_400_000, now - 3, now - 2, now - 1] {
        store.record_history(&record("a", started))?;
    }
    drop(store);
    let store = LocalStore::open(&path)?;
    let mut policy = HistoryRetention {
        max_entries: 1,
        max_age_days: 1,
        keep_forever: true,
    };
    assert_eq!(store.set_history_retention(policy, now)?, 0);
    assert_eq!(store.search_history(None, "", 10)?.len(), 4);
    policy.keep_forever = false;
    assert_eq!(store.set_history_retention(policy, now)?, 3);
    assert_eq!(store.search_history(None, "", 10)?.len(), 1);
    assert_eq!(store.clear_all_history()?, 1);
    assert!(store.search_history(None, "SELECT", 10)?.is_empty());
    Ok(())
}
