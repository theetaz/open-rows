// SPDX-License-Identifier: Apache-2.0
//! M1-08 against a real server: keepalive detection, transparent reconnect
//! with the search path restored, and the never-retry rule for a statement
//! that was running when its backend was terminated or the link was cut.
use super::{CountRows, TestResult, config, events, open, single_label};
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_core::{
    QueryEvent, Reconnect, Resilience, RetryPolicy, SessionEvent, SessionManager, SessionState,
};
use openrows_driver_api::{DriverError, ResultSink, Session, SinkFlow};
use openrows_driver_postgres::{BoundEdit, PgConfig, PgSession};
use openrows_net::testing::FaultyLink;
use openrows_results::ResultStore;
use openrows_sql::Dialect;
use openrows_types::SessionId;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast::Receiver;

/// Counts the rows of a statement run straight on a driver session.
#[derive(Default)]
struct Rows(usize);

impl ResultSink for Rows {
    fn begin_result(&mut self, _schema: SchemaRef) {}

    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        self.0 += batch.num_rows();
        SinkFlow::Continue
    }
}

pub(super) const KEEPALIVE: Duration = Duration::from_millis(500);
pub(super) const PROBE_TIMEOUT: Duration = Duration::from_millis(500);

pub(super) fn reconnect(config: PgConfig) -> Reconnect {
    let config = Arc::new(config);
    Arc::new(move || {
        let config = Arc::clone(&config);
        Box::pin(async move {
            let session: Box<dyn Session> = Box::new(PgSession::connect(&config).await?);
            Ok(session)
        })
    })
}

async fn resilient(manager: &SessionManager, config: PgConfig) -> TestResult<SessionId> {
    manager.set_keepalive(Some(KEEPALIVE));
    let resilience = Resilience {
        reconnect: reconnect(config),
        dialect: Some(Dialect::Postgres),
        policy: RetryPolicy {
            probe_timeout: PROBE_TIMEOUT,
            first_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(400),
            attempts: 10,
            ..RetryPolicy::default()
        },
        setup: Vec::new(),
    };
    Ok(manager.open_resilient(resilience).await?)
}

/// The next transition of `session` to `state`, within 20 s.
pub(super) async fn reaches(
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
    tokio::time::timeout(Duration::from_secs(20), wait).await?
}

pub(super) async fn backend_pid(
    manager: &SessionManager,
    store: &ResultStore,
    session: SessionId,
) -> TestResult<String> {
    let sql = "SELECT 1 AS n, pg_backend_pid()::text AS label";
    Ok(single_label(manager, store, session, sql)
        .await?
        .pop()
        .ok_or("no pid")?)
}

/// Terminate backend `pid` from a separate, plain session.
pub(super) async fn terminate(manager: &SessionManager, pid: &str) -> TestResult {
    let killer = open(manager).await?;
    let mut kill = manager.execute(killer, format!("SELECT pg_terminate_backend({pid})"))?;
    events(&mut kill).await?;
    manager.close(killer).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
async fn a_terminated_idle_backend_is_replaced_with_the_search_path_restored() -> TestResult {
    let store = Arc::new(ResultStore::default());
    let manager = SessionManager::new(store.clone());
    let mut changes = manager.subscribe();
    let session = resilient(&manager, config()?).await?;
    let mut set = manager.execute(session, "SET search_path TO m108_app, public")?;
    events(&mut set).await?;
    let before = backend_pid(&manager, &store, session).await?;
    terminate(&manager, &before).await?;
    let killed = Instant::now();
    reaches(&mut changes, session, SessionState::Reconnecting).await?;
    let detected = killed.elapsed();
    assert!(detected <= KEEPALIVE + PROBE_TIMEOUT, "{detected:?}");
    reaches(&mut changes, session, SessionState::Idle).await?;
    let after = backend_pid(&manager, &store, session).await?;
    assert_ne!(before, after, "a new backend serves the session");
    let sql = "SELECT 1 AS n, current_setting('search_path') AS label";
    let path = single_label(&manager, &store, session, sql).await?;
    assert_eq!(path, ["m108_app, public"]);
    manager.close(session).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
async fn a_write_running_when_its_backend_dies_fails_and_is_never_retried() -> TestResult {
    let store = Arc::new(ResultStore::default());
    let manager = SessionManager::new(store.clone());
    let mut changes = manager.subscribe();
    let setup = open(&manager).await?;
    let mut create = manager.execute(setup, "CREATE TABLE m108_inflight (id int)")?;
    events(&mut create).await?;
    let session = resilient(&manager, config()?).await?;
    let pid = backend_pid(&manager, &store, session).await?;
    let mut insert = manager.execute(
        session,
        "INSERT INTO m108_inflight SELECT 1 FROM pg_sleep(2)",
    )?;
    assert!(matches!(
        insert.next_event().await,
        Some(QueryEvent::Started { .. })
    ));
    terminate(&manager, &pid).await?;
    let Some(QueryEvent::Failed { error }) = events(&mut insert).await?.pop() else {
        return Err("the insert must fail".into());
    };
    assert!(
        error.message.contains("may or may not have been applied"),
        "{}",
        error.message
    );
    reaches(&mut changes, session, SessionState::Idle).await?;
    let sql = "SELECT 1 AS n, count(*)::text AS label FROM m108_inflight";
    assert_eq!(single_label(&manager, &store, session, sql).await?, ["0"]);
    let mut drop = manager.execute(setup, "DROP TABLE m108_inflight")?;
    events(&mut drop).await?;
    manager.close(session).await?;
    manager.close(setup).await?;
    Ok(())
}

async fn link_to_server() -> TestResult<(FaultyLink, PgConfig)> {
    let config = config()?;
    let server = SocketAddr::from((Ipv4Addr::LOCALHOST, config.port));
    let link = FaultyLink::start(server).await?;
    let through = PgConfig {
        port: link.port,
        ..config
    };
    Ok((link, through))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
async fn a_silent_network_is_detected_within_the_keepalive_interval() -> TestResult {
    let manager = SessionManager::new(Arc::new(CountRows::default()));
    let mut changes = manager.subscribe();
    let (link, through) = link_to_server().await?;
    let session = resilient(&manager, through).await?;
    link.set_blackhole(true);
    let silent = Instant::now();
    let lost = reaches(&mut changes, session, SessionState::Reconnecting).await?;
    let detected = silent.elapsed();
    assert!(
        detected <= KEEPALIVE + PROBE_TIMEOUT + Duration::from_millis(250),
        "{detected:?}"
    );
    let reason = lost.error.map(|error| error.message).unwrap_or_default();
    assert!(reason.contains("keepalive probe"), "{reason}");
    // The network comes back; connections stuck in the outage are dropped.
    link.set_blackhole(false);
    link.cut();
    reaches(&mut changes, session, SessionState::Idle).await?;
    let mut after = manager.execute(session, "SELECT 1")?;
    assert!(matches!(
        events(&mut after).await?.pop(),
        Some(QueryEvent::Completed { .. })
    ));
    manager.close(session).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
async fn a_server_that_goes_away_and_comes_back_is_reconnected() -> TestResult {
    let manager = SessionManager::new(Arc::new(CountRows::default()));
    let mut changes = manager.subscribe();
    let (link, through) = link_to_server().await?;
    let session = resilient(&manager, through).await?;
    link.set_refusing(true);
    link.cut();
    reaches(&mut changes, session, SessionState::Reconnecting).await?;
    let mut queued = manager.execute(session, "SELECT 1")?;
    link.set_refusing(false);
    reaches(&mut changes, session, SessionState::Idle).await?;
    let outcome = events(&mut queued).await?.pop();
    assert!(
        matches!(outcome, Some(QueryEvent::Completed { .. })),
        "a statement queued during the outage runs once the server is back: {outcome:?}"
    );
    manager.close(session).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
async fn an_edit_commit_cut_mid_transaction_reports_a_rollback() -> TestResult {
    let mut admin = PgSession::connect(&config()?).await?;
    let mut discard = Rows::default();
    admin
        .execute("CREATE TABLE m108_edits (id int PRIMARY KEY)", &mut discard)
        .await?;
    let mut editor = PgSession::connect(&config()?).await?;
    let edits = [
        BoundEdit {
            sql: "INSERT INTO m108_edits VALUES (1)".to_owned(),
            params: Vec::new(),
        },
        BoundEdit {
            sql: "SELECT pg_terminate_backend(pg_backend_pid())".to_owned(),
            params: Vec::new(),
        },
    ];
    let failed = editor.commit_edits(&edits).await.err().ok_or("committed")?;
    assert!(matches!(failed, DriverError::Network(_)), "{failed}");
    assert!(
        failed.to_string().contains("rolled the transaction back"),
        "{failed}"
    );
    let mut count = Rows::default();
    admin
        .execute("SELECT * FROM m108_edits", &mut count)
        .await?;
    assert_eq!(count.0, 0, "nothing of the batch was applied");
    admin.execute("DROP TABLE m108_edits", &mut discard).await?;
    Ok(())
}
