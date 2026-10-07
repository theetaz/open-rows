// SPDX-License-Identifier: Apache-2.0
//! Out-of-band cancellation of running and queued statements.
use super::{FakeCancel, TestResult, last_event, manager, open, started};
use crate::{ErrorKind, QueryEvent, SessionState};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

#[tokio::test]
async fn cancel_reaches_the_driver_within_200_ms() -> TestResult {
    let (manager, _store) = manager();
    let cancel = Arc::new(FakeCancel::default());
    let session = open(&manager, Some(cancel.clone())).await?;
    let mut query = manager.execute(session, "hang")?;
    started(&mut query).await?;
    let requested = Instant::now();
    query.canceller().cancel();
    assert!(matches!(
        last_event(&mut query).await?,
        QueryEvent::Cancelled
    ));
    assert!(requested.elapsed() < Duration::from_millis(200));
    assert_eq!(cancel.requests.load(Ordering::SeqCst), 1);
    let mut next = manager.execute(session, "rows 1 1")?;
    assert!(matches!(
        last_event(&mut next).await?,
        QueryEvent::Completed { .. }
    ));
    Ok(())
}

#[tokio::test]
async fn queued_queries_cancel_without_starting() -> TestResult {
    let (manager, _store) = manager();
    let cancel = Arc::new(FakeCancel::default());
    let session = open(&manager, Some(cancel.clone())).await?;
    let mut running = manager.execute(session, "sleep 20")?;
    let mut queued = manager.execute(session, "rows 1 1")?;
    queued.canceller().cancel();
    assert!(matches!(
        queued.next_event().await,
        Some(QueryEvent::Cancelled)
    ));
    assert!(matches!(
        last_event(&mut running).await?,
        QueryEvent::Completed { .. }
    ));
    assert_eq!(cancel.requests.load(Ordering::SeqCst), 0);
    Ok(())
}

#[tokio::test]
async fn cancel_without_driver_support_discards_the_connection() -> TestResult {
    let (manager, _store) = manager();
    let session = open(&manager, None).await?;
    let mut query = manager.execute(session, "hang")?;
    started(&mut query).await?;
    query.canceller().cancel();
    assert!(matches!(
        last_event(&mut query).await?,
        QueryEvent::Cancelled
    ));
    tokio::task::yield_now().await;
    assert_eq!(manager.state(session), Some(SessionState::Failed));
    Ok(())
}

#[tokio::test]
async fn close_cancels_running_and_queued_queries() -> TestResult {
    let (manager, _store) = manager();
    let cancel = Arc::new(FakeCancel::default());
    let session = open(&manager, Some(cancel)).await?;
    let mut running = manager.execute(session, "hang")?;
    let mut queued = manager.execute(session, "rows 1 1")?;
    started(&mut running).await?;
    manager.close(session).await?;
    assert!(matches!(
        last_event(&mut running).await?,
        QueryEvent::Cancelled
    ));
    assert!(matches!(
        last_event(&mut queued).await?,
        QueryEvent::Cancelled
    ));
    Ok(())
}

#[tokio::test]
async fn a_full_queue_rejects_new_work() -> TestResult {
    let (manager, _store) = manager();
    let cancel = Arc::new(FakeCancel::default());
    let session = open(&manager, Some(cancel)).await?;
    let mut running = manager.execute(session, "hang")?;
    started(&mut running).await?;
    let queued: Vec<_> = (0..32)
        .map(|_| manager.execute(session, "rows 1 1"))
        .collect::<Result<_, _>>()?;
    let refused = manager
        .execute(session, "rows 1 1")
        .err()
        .ok_or("accepted")?;
    assert_eq!(refused.kind, ErrorKind::QueueFull);
    running.canceller().cancel();
    drop(queued);
    Ok(())
}
