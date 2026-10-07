// SPDX-License-Identifier: Apache-2.0
//! Session actor tests against the scripted [`fake::Fake`] driver.
mod cancel;
mod fake;
mod isolation;
mod lifecycle;

use crate::{QueryEvent, QueryHandle, SessionManager};
use fake::{Fake, FakeCancel, Store};
use openrows_types::SessionId;
use std::sync::Arc;
use std::time::Duration;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// A manager over a fresh store, so tests can inspect delivered batches.
fn manager() -> (SessionManager, Arc<Store>) {
    let store = Arc::new(Store::default());
    (SessionManager::new(store.clone()), store)
}

async fn open(manager: &SessionManager, cancel: Option<Arc<FakeCancel>>) -> TestResult<SessionId> {
    Ok(manager.open(async move { Ok(Fake::boxed(cancel)) }).await?)
}

/// Every event up to and including the terminal one; fails after 5 s.
async fn events(handle: &mut QueryHandle) -> TestResult<Vec<QueryEvent>> {
    let mut seen = Vec::new();
    loop {
        let event = tokio::time::timeout(Duration::from_secs(5), handle.next_event())
            .await?
            .ok_or("event stream ended without a terminal event")?;
        let terminal = event.is_terminal();
        seen.push(event);
        if terminal {
            return Ok(seen);
        }
    }
}

async fn last_event(handle: &mut QueryHandle) -> TestResult<QueryEvent> {
    Ok(events(handle).await?.pop().ok_or("no events")?)
}

/// Wait until the next event; used to know a query is running.
async fn started(handle: &mut QueryHandle) -> TestResult {
    match handle.next_event().await {
        Some(QueryEvent::Started { .. }) => Ok(()),
        other => Err(format!("expected Started, got {other:?}").into()),
    }
}
