// SPDX-License-Identifier: Apache-2.0
//! Open, execute, events, errors and close on a healthy session.
use super::{TestResult, events, last_event, manager, open, started};
use crate::{ErrorKind, QueryEvent, SessionEvent, SessionState};
use openrows_driver_api::{DriverError, NoticeLevel};

#[tokio::test]
async fn query_streams_batches_and_ordered_events() -> TestResult {
    let (manager, store) = manager();
    let session = open(&manager, None).await?;
    assert_eq!(manager.state(session), Some(SessionState::Idle));
    let mut query = manager.execute(session, "rows 3 10")?;
    let seen = events(&mut query).await?;
    assert!(matches!(seen[0], QueryEvent::Started { query: id } if id == query.id()));
    assert!(matches!(seen[1], QueryEvent::ResultBegin { .. }));
    assert!(matches!(seen[2], QueryEvent::Progress { rows: 10, .. }));
    let end = &seen[seen.len() - 2];
    assert!(matches!(
        end,
        QueryEvent::ResultEnd {
            rows: 30,
            truncated: false,
            ..
        }
    ));
    assert!(matches!(seen.last(), Some(QueryEvent::Completed { .. })));
    assert_eq!((store.batches(), store.ended()), (3, 1));
    assert_eq!(manager.state(session), Some(SessionState::Idle));
    Ok(())
}

#[tokio::test]
async fn progress_is_coalesced() -> TestResult {
    let (manager, store) = manager();
    let session = open(&manager, None).await?;
    let mut query = manager.execute(session, "rows 5000 1")?;
    let seen = events(&mut query).await?;
    let progress = seen
        .iter()
        .filter(|event| matches!(event, QueryEvent::Progress { .. }))
        .count();
    assert_eq!(store.batches(), 5_000);
    assert!((1..=3).contains(&progress), "{progress} progress events");
    Ok(())
}

#[tokio::test]
async fn sink_stop_truncates_the_result() -> TestResult {
    let store = std::sync::Arc::new(super::Store::limited(2));
    let manager = crate::SessionManager::new(store.clone());
    let session = open(&manager, None).await?;
    let mut query = manager.execute(session, "rows 10 1")?;
    let seen = events(&mut query).await?;
    let end = &seen[seen.len() - 2];
    assert!(matches!(
        end,
        QueryEvent::ResultEnd {
            rows: 2,
            truncated: true,
            ..
        }
    ));
    assert_eq!(store.batches(), 2);
    Ok(())
}

#[tokio::test]
async fn rows_affected_and_notices_are_reported() -> TestResult {
    let (manager, _store) = manager();
    let session = open(&manager, None).await?;
    let mut update = manager.execute(session, "update")?;
    let mut notice = manager.execute(session, "notice")?;
    let seen = events(&mut update).await?;
    assert!(matches!(seen[1], QueryEvent::RowsAffected { count: 3 }));
    let seen = events(&mut notice).await?;
    assert!(matches!(
        &seen[1],
        QueryEvent::Notice { level: NoticeLevel::Warning, message } if message == "careful"
    ));
    Ok(())
}

#[tokio::test]
async fn server_errors_keep_the_session_usable() -> TestResult {
    let (manager, _store) = manager();
    let session = open(&manager, None).await?;
    let mut failing = manager.execute(session, "fail")?;
    let QueryEvent::Failed { error } = last_event(&mut failing).await? else {
        return Err("expected Failed".into());
    };
    assert_eq!(
        (error.kind, error.db_code.as_deref()),
        (ErrorKind::Server, Some("42601"))
    );
    let mut next = manager.execute(session, "rows 1 1")?;
    assert!(matches!(
        last_event(&mut next).await?,
        QueryEvent::Completed { .. }
    ));
    assert_eq!(manager.state(session), Some(SessionState::Idle));
    Ok(())
}

#[tokio::test]
async fn transport_errors_fail_only_that_session() -> TestResult {
    let (manager, _store) = manager();
    let mut changes = manager.subscribe();
    let session = open(&manager, None).await?;
    let mut lost = manager.execute(session, "drop")?;
    let QueryEvent::Failed { error } = last_event(&mut lost).await? else {
        return Err("expected Failed".into());
    };
    assert_eq!(error.kind, ErrorKind::Network);
    let failed = loop {
        let change: SessionEvent = changes.recv().await?;
        if change.state == SessionState::Failed {
            break change;
        }
    };
    assert_eq!(failed.session, session);
    assert_eq!(manager.state(session), Some(SessionState::Failed));
    let refused = manager
        .execute(session, "rows 1 1")
        .err()
        .ok_or("accepted")?;
    assert_eq!(refused.kind, ErrorKind::SessionClosed);
    Ok(())
}

#[tokio::test]
async fn state_is_busy_while_a_statement_runs() -> TestResult {
    let (manager, _store) = manager();
    let cancel = std::sync::Arc::default();
    let session = open(&manager, Some(cancel)).await?;
    let mut query = manager.execute(session, "hang")?;
    started(&mut query).await?;
    assert_eq!(manager.state(session), Some(SessionState::Busy));
    query.canceller().cancel();
    assert!(matches!(
        last_event(&mut query).await?,
        QueryEvent::Cancelled
    ));
    assert_eq!(manager.state(session), Some(SessionState::Idle));
    Ok(())
}

#[tokio::test]
async fn connect_failures_are_typed_and_forgotten() -> TestResult {
    let (manager, _store) = manager();
    let error = manager
        .open(async { Err(DriverError::Auth("password rejected".to_owned())) })
        .await
        .err()
        .ok_or("connected")?;
    assert_eq!(error.kind, ErrorKind::Auth);
    Ok(())
}

#[tokio::test]
async fn close_forgets_the_session() -> TestResult {
    let (manager, _store) = manager();
    let mut changes = manager.subscribe();
    let session = open(&manager, None).await?;
    manager.close(session).await?;
    assert_eq!(manager.state(session), None);
    let refused = manager
        .execute(session, "rows 1 1")
        .err()
        .ok_or("accepted")?;
    assert_eq!(refused.kind, ErrorKind::UnknownSession);
    let states: Vec<SessionState> = std::iter::from_fn(|| changes.try_recv().ok())
        .map(|change| change.state)
        .collect();
    let expected = [
        SessionState::Connecting,
        SessionState::Idle,
        SessionState::Closed,
    ];
    assert_eq!(states, expected);
    Ok(())
}
