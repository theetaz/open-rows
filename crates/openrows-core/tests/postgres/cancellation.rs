// SPDX-License-Identifier: Apache-2.0
//! Observe server execution before sending the single out-of-band cancel.
use super::{TestResult, events, open, single_label};
use openrows_core::{QueryEvent, SessionManager};
use openrows_results::ResultStore;
use openrows_types::SessionId;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// `Started` precedes protocol preparation; it does not mean the server is running SQL.
pub(super) async fn cancel_running_query(
    manager: &SessionManager,
    session: SessionId,
) -> TestResult {
    let sql = format!("SELECT pg_sleep(30) /* cancel {session} */");
    let mut query = manager.execute(session, &sql)?;
    assert!(matches!(
        query.next_event().await,
        Some(QueryEvent::Started { .. })
    ));
    wait_for_sleep(&sql).await?;
    let requested = Instant::now();
    query.canceller().cancel();
    assert!(matches!(
        events(&mut query).await?.pop(),
        Some(QueryEvent::Cancelled)
    ));
    assert!(
        requested.elapsed() < Duration::from_secs(2),
        "{:?}",
        requested.elapsed()
    );
    Ok(())
}

/// Poll on an independent session; the unique SQL identifies only this test's backend.
async fn wait_for_sleep(sql: &str) -> TestResult {
    let store = Arc::new(ResultStore::default());
    let observer = SessionManager::new(store.clone());
    let session = open(&observer).await?;
    let check = format!(
        "SELECT 1 AS n, count(*)::text AS label FROM pg_stat_activity \
         WHERE query = '{sql}' AND state = 'active' AND wait_event = 'PgSleep'"
    );
    let ready = async {
        loop {
            if single_label(&observer, &store, session, &check).await? == ["1"] {
                return Ok::<_, Box<dyn std::error::Error>>(());
            }
        }
    };
    // Real server startup is bounded separately from the unchanged cancel latency budget.
    let result = tokio::time::timeout(Duration::from_secs(10), ready).await;
    observer.close(session).await?;
    result??;
    Ok(())
}
