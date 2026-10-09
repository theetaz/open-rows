// SPDX-License-Identifier: Apache-2.0
//! Statement timeouts and idle suspension (M1-01): a statement past its
//! timeout is cancelled through the cancel path and reported as a timeout,
//! not a user cancel; an idle resilient session closes its connection and
//! reconnects (setup and settings replayed) for the next statement.
use super::flaky::FlakyServer;
use super::{FakeCancel, TestResult, events, last_event, manager, open, started};
use crate::{ErrorKind, QueryEvent, SessionEvent, SessionManager, SessionState};
use openrows_types::SessionId;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::broadcast::Receiver;

const LIMIT: Duration = Duration::from_millis(50);
const IDLE: Duration = Duration::from_secs(60);

fn failure(event: &QueryEvent) -> Option<ErrorKind> {
    match event {
        QueryEvent::Failed { error } => Some(error.kind),
        _ => None,
    }
}

#[tokio::test]
async fn a_statement_past_its_timeout_is_cancelled_and_reported_as_a_timeout() -> TestResult {
    let (manager, _store) = manager();
    let cancel = Arc::new(FakeCancel::default());
    let session = open(&manager, Some(cancel.clone())).await?;
    manager.set_statement_timeout(session, Some(LIMIT))?;
    let mut query = manager.execute(session, "hang")?;
    let last = last_event(&mut query).await?;
    assert_eq!(failure(&last), Some(ErrorKind::Timeout), "{last:?}");
    if let QueryEvent::Failed { error } = &last {
        assert!(error.message.contains("50 ms statement timeout"), "{error}");
    }
    assert_eq!(cancel.requests.load(Ordering::SeqCst), 1);
    assert_eq!(manager.state(session), Some(SessionState::Idle));
    let mut next = manager.execute(session, "rows 1 1")?;
    assert!(matches!(
        last_event(&mut next).await?,
        QueryEvent::Completed { .. }
    ));
    Ok(())
}

#[tokio::test]
async fn a_user_cancel_stays_a_cancel_and_no_limit_never_fires() -> TestResult {
    let (manager, _store) = manager();
    let cancel = Arc::new(FakeCancel::default());
    let session = open(&manager, Some(cancel)).await?;
    manager.set_statement_timeout(session, Some(Duration::from_secs(30)))?;
    let mut query = manager.execute(session, "hang")?;
    started(&mut query).await?;
    query.canceller().cancel();
    assert!(matches!(
        last_event(&mut query).await?,
        QueryEvent::Cancelled
    ));
    manager.set_statement_timeout(session, Some(LIMIT))?;
    let mut hung = manager.execute(session, "hang")?;
    assert_eq!(
        failure(&last_event(&mut hung).await?),
        Some(ErrorKind::Timeout)
    );
    manager.set_statement_timeout(session, None)?;
    let mut slow = manager.execute(session, "sleep 100")?;
    assert!(matches!(
        last_event(&mut slow).await?,
        QueryEvent::Completed { .. }
    ));
    let unknown = SessionId::new();
    assert!(manager.set_statement_timeout(unknown, None).is_err());
    Ok(())
}

async fn suspendable(manager: &SessionManager) -> TestResult<(Arc<FlakyServer>, SessionId)> {
    manager.set_keepalive(None);
    let server = Arc::new(FlakyServer::default());
    let mut recipe = server.resilience();
    recipe.setup = vec!["set statement_timeout = 5000".to_owned()];
    let session = manager.open_resilient(recipe).await?;
    Ok((server, session))
}

async fn reaches(
    changes: &mut Receiver<SessionEvent>,
    session: SessionId,
    state: SessionState,
) -> TestResult {
    let wait = async {
        loop {
            let change = changes.recv().await?;
            if change.session == session && change.state == state {
                return Ok::<_, Box<dyn std::error::Error>>(());
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(5), wait).await?
}

async fn run(manager: &SessionManager, session: SessionId, sql: &str) -> TestResult<QueryEvent> {
    last_event(&mut manager.execute(session, sql)?).await
}

#[tokio::test(start_paused = true)]
async fn an_idle_session_suspends_and_reconnects_with_its_settings_on_next_use() -> TestResult {
    let (manager, _store) = manager();
    let mut changes = manager.subscribe();
    let (server, session) = suspendable(&manager).await?;
    run(&manager, session, "SET search_path TO app").await?;
    manager.suspend_idle(session, IDLE)?;
    run(&manager, session, "select 1").await?;
    assert_eq!(
        server.connects(),
        1,
        "a recently used session was suspended"
    );

    tokio::time::advance(IDLE).await;
    manager.suspend_idle(session, IDLE)?;
    reaches(&mut changes, session, SessionState::Suspended).await?;
    assert_eq!(manager.state(session), Some(SessionState::Suspended));
    let woke = manager.execute(session, "select 2")?;
    reaches(&mut changes, session, SessionState::Connecting).await?;
    let mut woke = woke;
    assert!(matches!(
        events(&mut woke).await?.pop(),
        Some(QueryEvent::Completed { .. })
    ));
    assert_eq!(server.connects(), 2);
    assert_eq!(
        server.applied(),
        [
            "set statement_timeout = 5000",
            "SET search_path TO app",
            "select 1",
            "set statement_timeout = 5000",
            "SET search_path TO app",
            "select 2",
        ]
    );
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn an_open_transaction_keeps_the_connection_and_close_ends_a_suspended_session() -> TestResult
{
    let (manager, _store) = manager();
    let mut changes = manager.subscribe();
    let (server, session) = suspendable(&manager).await?;
    run(&manager, session, "begin").await?;
    tokio::time::advance(IDLE).await;
    manager.suspend_idle(session, IDLE)?;
    run(&manager, session, "commit").await?;
    assert_eq!(server.connects(), 1, "a transaction was dropped");

    tokio::time::advance(IDLE).await;
    manager.suspend_idle(session, IDLE)?;
    reaches(&mut changes, session, SessionState::Suspended).await?;
    manager.close(session).await?;
    assert_eq!(manager.state(session), None);
    assert_eq!(server.connects(), 1);
    Ok(())
}

#[tokio::test]
async fn sessions_without_a_reconnect_recipe_never_suspend() -> TestResult {
    let (manager, _store) = manager();
    let session = open(&manager, None).await?;
    manager.suspend_idle(session, Duration::ZERO)?;
    let mut query = manager.execute(session, "rows 1 1")?;
    assert!(matches!(
        last_event(&mut query).await?,
        QueryEvent::Completed { .. }
    ));
    assert_eq!(manager.state(session), Some(SessionState::Idle));
    Ok(())
}
