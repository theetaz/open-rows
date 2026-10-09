// SPDX-License-Identifier: Apache-2.0
//! Keepalive probes, transparent reconnects and the never-retry rule against
//! the scripted [`FlakyServer`], on paused (auto-advancing) time.
use super::flaky::FlakyServer;
use super::{TestResult, last_event, manager};
use crate::script::{Script, ScriptCanceller, ScriptOutcome, run_script};
use crate::{ErrorKind, QueryEvent, SessionEvent, SessionManager, SessionState};
use openrows_sql::select::BoundSql;
use openrows_types::SessionId;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::Receiver;

const KEEPALIVE: Duration = Duration::from_millis(50);

async fn resilient(manager: &SessionManager) -> TestResult<(Arc<FlakyServer>, SessionId)> {
    manager.set_keepalive(Some(KEEPALIVE));
    let server = Arc::new(FlakyServer::default());
    let session = manager.open_resilient(server.resilience()).await?;
    Ok((server, session))
}

/// The next transition of `session` to `state`; fails after 5 s (virtual).
async fn reaches(
    changes: &mut Receiver<SessionEvent>,
    session: SessionId,
    state: SessionState,
) -> TestResult<SessionEvent> {
    let wait = async {
        loop {
            let change = changes.recv().await?;
            if change.session == session && change.state == state {
                return Ok::<_, Box<dyn std::error::Error>>(change);
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(5), wait).await?
}

/// A full reconnect: `Reconnecting`, then `Idle` on the new connection.
async fn reconnects(changes: &mut Receiver<SessionEvent>, session: SessionId) -> TestResult {
    reaches(changes, session, SessionState::Reconnecting).await?;
    reaches(changes, session, SessionState::Idle).await?;
    Ok(())
}

async fn run(manager: &SessionManager, session: SessionId, sql: &str) -> TestResult<QueryEvent> {
    last_event(&mut manager.execute(session, sql)?).await
}

#[tokio::test(start_paused = true)]
async fn a_lost_idle_connection_is_detected_within_the_keepalive_interval() -> TestResult {
    let (manager, _store) = manager();
    let mut changes = manager.subscribe();
    let (server, session) = resilient(&manager).await?;
    let lost_at = tokio::time::Instant::now();
    server.kill_connections();
    let reconnecting = reaches(&mut changes, session, SessionState::Reconnecting).await?;
    assert!(lost_at.elapsed() <= KEEPALIVE, "{:?}", lost_at.elapsed());
    assert_eq!(
        reconnecting.error.map(|error| error.kind),
        Some(ErrorKind::Network)
    );
    reaches(&mut changes, session, SessionState::Idle).await?;
    assert_eq!(server.connects(), 2);
    assert!(matches!(
        run(&manager, session, "select").await?,
        QueryEvent::Completed { .. }
    ));
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn a_half_open_connection_fails_its_probe_and_is_replaced() -> TestResult {
    let (manager, _store) = manager();
    let mut changes = manager.subscribe();
    let (server, session) = resilient(&manager).await?;
    server.set_silent(true);
    let reconnecting = reaches(&mut changes, session, SessionState::Reconnecting).await?;
    let message = reconnecting
        .error
        .map(|error| error.message)
        .unwrap_or_default();
    assert!(message.contains("keepalive probe"), "{message}");
    server.set_silent(false);
    reaches(&mut changes, session, SessionState::Idle).await?;
    assert_eq!(server.connects(), 2);
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn a_statement_in_flight_fails_as_uncertain_and_is_never_retried() -> TestResult {
    let (manager, _store) = manager();
    let mut changes = manager.subscribe();
    let (server, session) = resilient(&manager).await?;
    let QueryEvent::Failed { error } = run(&manager, session, "kill-after insert").await? else {
        return Err("expected Failed".into());
    };
    assert_eq!(error.kind, ErrorKind::Network);
    assert!(
        error.message.contains("may or may not have been applied"),
        "{}",
        error.message
    );
    reconnects(&mut changes, session).await?;
    assert!(matches!(
        run(&manager, session, "select").await?,
        QueryEvent::Completed { .. }
    ));
    assert_eq!(
        server.applied(),
        ["insert", "select"],
        "the insert ran exactly once"
    );
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn a_lost_connection_ends_a_script_even_without_stop_on_error() -> TestResult {
    let (manager, _store) = manager();
    let (server, session) = resilient(&manager).await?;
    let statements: Vec<BoundSql> = ["kill-after update a", "update b"]
        .into_iter()
        .map(|sql| BoundSql {
            sql: sql.to_owned(),
            params: Vec::new(),
        })
        .collect();
    let script = Script {
        statements: &statements,
        max_rows: None,
        stop_on_error: false,
        sink: None,
    };
    let outcome = run_script(&manager, session, script, &ScriptCanceller::default(), drop).await;
    assert_eq!(outcome, ScriptOutcome::Failed);
    assert_eq!(server.applied(), ["update a"], "nothing after the loss ran");
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn losing_an_open_transaction_fails_the_session_instead_of_reconnecting() -> TestResult {
    let (manager, _store) = manager();
    let mut changes = manager.subscribe();
    let (server, session) = resilient(&manager).await?;
    assert!(matches!(
        run(&manager, session, "begin").await?,
        QueryEvent::Completed { .. }
    ));
    server.kill_connections();
    let failed = reaches(&mut changes, session, SessionState::Failed).await?;
    let message = failed.error.map(|error| error.message).unwrap_or_default();
    assert!(message.contains("rolled the transaction back"), "{message}");
    assert_eq!(
        server.connects(),
        1,
        "no silent reconnect outside the transaction"
    );
    let refused = manager.execute(session, "insert").err().ok_or("accepted")?;
    assert_eq!(refused.kind, ErrorKind::SessionClosed);
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn a_commit_in_flight_inside_a_transaction_fails_the_session() -> TestResult {
    let (manager, _store) = manager();
    let mut changes = manager.subscribe();
    let (server, session) = resilient(&manager).await?;
    run(&manager, session, "begin").await?;
    let QueryEvent::Failed { error } = run(&manager, session, "kill-after commit").await? else {
        return Err("expected Failed".into());
    };
    assert!(
        error
            .message
            .contains("unless this statement was its COMMIT"),
        "{}",
        error.message
    );
    reaches(&mut changes, session, SessionState::Failed).await?;
    assert_eq!(server.connects(), 1);
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn the_search_path_is_replayed_on_the_new_connection() -> TestResult {
    let (manager, _store) = manager();
    let mut changes = manager.subscribe();
    let (server, session) = resilient(&manager).await?;
    run(&manager, session, "SET search_path TO app, public").await?;
    run(&manager, session, "SET search_path TO reports").await?;
    server.kill_connections();
    reconnects(&mut changes, session).await?;
    let applied = server.applied();
    assert_eq!(
        applied.last().map(String::as_str),
        Some("SET search_path TO reports")
    );
    assert_eq!(
        applied.len(),
        3,
        "only the latest search path is replayed: {applied:?}"
    );
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn reconnecting_gives_up_after_the_configured_attempts() -> TestResult {
    let (manager, _store) = manager();
    let mut changes = manager.subscribe();
    let (server, session) = resilient(&manager).await?;
    server.set_refusing(true);
    server.kill_connections();
    reaches(&mut changes, session, SessionState::Reconnecting).await?;
    let mut waiting = manager.execute(session, "select")?;
    let failed = reaches(&mut changes, session, SessionState::Failed).await?;
    let message = failed.error.map(|error| error.message).unwrap_or_default();
    assert!(message.contains("after 3 attempts"), "{message}");
    let QueryEvent::Failed { error } = last_event(&mut waiting).await? else {
        return Err("the queued statement must fail, not hang".into());
    };
    assert!(error.message.contains("could not be re-established"));
    assert!(server.applied().is_empty());
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn a_wake_probes_idle_sessions_at_once() -> TestResult {
    let (manager, _store) = manager();
    let mut changes = manager.subscribe();
    let (server, session) = resilient(&manager).await?;
    manager.set_keepalive(None);
    server.kill_connections();
    let quiet = reaches(&mut changes, session, SessionState::Reconnecting);
    assert!(quiet.await.is_err(), "without keepalive nothing probes");
    manager.probe_all();
    reconnects(&mut changes, session).await?;
    assert_eq!(server.connects(), 2);
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn a_stale_session_is_reopened_before_the_next_statement_runs() -> TestResult {
    let (manager, _store) = manager();
    let server = Arc::new(FlakyServer::default());
    let mut resilience = server.resilience();
    resilience.policy.stale_after = Duration::from_millis(20);
    manager.set_keepalive(Some(Duration::from_secs(3600)));
    let session = manager.open_resilient(resilience).await?;
    tokio::time::sleep(Duration::from_millis(30)).await;
    server.kill_connections();
    assert!(matches!(
        run(&manager, session, "insert").await?,
        QueryEvent::Completed { .. }
    ));
    assert_eq!(server.applied(), ["insert"]);
    assert_eq!(server.connects(), 2);
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn closing_while_reconnecting_stops_at_once() -> TestResult {
    let (manager, _store) = manager();
    let mut changes = manager.subscribe();
    let (server, session) = resilient(&manager).await?;
    server.set_refusing(true);
    server.kill_connections();
    reaches(&mut changes, session, SessionState::Reconnecting).await?;
    tokio::time::timeout(Duration::from_millis(5), manager.close(session)).await??;
    reaches(&mut changes, session, SessionState::Closed).await?;
    assert_eq!(manager.state(session), None);
    Ok(())
}
