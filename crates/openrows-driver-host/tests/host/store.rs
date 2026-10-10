#![allow(
    clippy::panic,
    reason = "unexpected synthetic query failure is a test assertion"
)]
// SPDX-License-Identifier: Apache-2.0
//! The actual core/store admission hook pauses spill and marks failed-spill truncation.
use crate::support;
use openrows_core::{QueryEvent, SessionManager};
use openrows_driver_api::Session;
use openrows_results::{Limits, ResultStore, SpillDir};
use std::{sync::Arc, time::Duration};
#[tokio::test]
async fn hosted_store_spills_one_million_rows_with_bounded_memory() {
    let _permit = support::host_test().await.unwrap();
    store(false).await;
}
#[tokio::test]
async fn hosted_failed_spill_stops_partial_rows_and_keeps_session_usable() {
    let _permit = support::host_test().await.unwrap();
    store(true).await;
}
async fn store(failed: bool) {
    let root = tempfile::tempdir().unwrap();
    let dir = SpillDir::create(root.path()).unwrap();
    let path = dir.path().to_path_buf();
    let store = Arc::new(
        ResultStore::with_spill(
            Limits {
                max_rows: usize::MAX,
                max_bytes: 1024 * 1024,
            },
            dir,
        )
        .unwrap(),
    );
    if failed {
        std::fs::remove_dir_all(path).unwrap();
    }
    let manager = SessionManager::new(store.clone());
    let connection = super::adapter::connection().await;
    let session = manager
        .open(async move { Ok(connection as Box<dyn Session>) })
        .await
        .unwrap();
    let mut query = manager.execute(session, "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<1000000) SELECT x FROM n").unwrap();
    let mut result = None;
    let mut completed = 0;
    tokio::time::timeout(Duration::from_secs(20), async {
        while let Some(event) = query.next_event().await {
            match event {
                QueryEvent::ResultBegin { result: id, .. } => result = Some(id),
                QueryEvent::Completed { .. } => completed += 1,
                QueryEvent::Failed { error } => panic!("unexpected failure {error:?}"),
                _ => {}
            }
            assert!(store.memory() <= 1024 * 1024 + openrows_host_protocol::BATCH_LIMIT);
        }
    })
    .await
    .unwrap();
    let (rows, truncated) = store.rows(&result.unwrap().to_string()).unwrap();
    assert_eq!(completed, 1);
    assert_eq!(truncated, failed);
    if failed {
        assert!(rows < 1_000_000);
    } else {
        assert_eq!(rows, 1_000_000);
    }
    let mut probe = manager.execute(session, "SELECT 1").unwrap();
    while let Some(event) = probe.next_event().await {
        assert!(!matches!(event, QueryEvent::Failed { .. }));
    }
    manager.close(session).await.unwrap();
    store.close();
}
