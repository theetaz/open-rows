// SPDX-License-Identifier: Apache-2.0
//! M1-08 against `MySQL`/`MariaDB`: a `KILL`ed idle connection is replaced
//! with its `USE` database restored, a statement killed while running is
//! reported and never retried, and an edit commit killed mid-transaction is
//! reported as rolled back.
use super::{TestResult, open, request, run};
use arrow_array::{RecordBatch, StringArray};
use arrow_schema::SchemaRef;
use openrows_core::{
    BatchSink, QueryEvent, Reconnect, Resilience, RetryPolicy, SessionEvent, SessionManager,
    SessionState,
};
use openrows_driver_api::{ConnectTrace, DriverError, Editor, ResultSink, Session, SinkFlow};
use openrows_sql::Dialect;
use openrows_sql::select::BoundSql;
use openrows_types::{ResultId, SessionId};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};
use tokio::sync::broadcast::Receiver;

const KEEPALIVE: Duration = Duration::from_millis(500);
const PROBE_TIMEOUT: Duration = Duration::from_millis(500);

/// Keeps the text values of every result's first column.
#[derive(Default)]
struct Texts(Mutex<Vec<String>>);

impl Texts {
    fn last(&self) -> Option<String> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .last()
            .cloned()
    }
}

impl BatchSink for Texts {
    fn begin(&self, _result: ResultId, _schema: SchemaRef) {}

    fn push(&self, _result: ResultId, batch: RecordBatch) -> SinkFlow {
        if let Some(column) = batch.column(0).as_any().downcast_ref::<StringArray>() {
            let mut texts = self.0.lock().unwrap_or_else(PoisonError::into_inner);
            texts.extend(column.iter().flatten().map(str::to_owned));
        }
        SinkFlow::Continue
    }

    fn end(&self, _result: ResultId) {}
}

/// The first column's text values of a statement run on a driver session.
#[derive(Default)]
struct Column(Vec<String>);

impl ResultSink for Column {
    fn begin_result(&mut self, _schema: SchemaRef) {}

    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        if let Some(column) = batch.column(0).as_any().downcast_ref::<StringArray>() {
            self.0.extend(column.iter().flatten().map(str::to_owned));
        }
        SinkFlow::Continue
    }
}

fn reconnect() -> TestResult<Reconnect> {
    let request = Arc::new(request()?);
    Ok(Arc::new(move || {
        let request = Arc::clone(&request);
        Box::pin(async move {
            let mut trace = ConnectTrace::default();
            let session = openrows_driver_mysql::connect(&request, &mut trace).await?;
            let session: Box<dyn Session> = Box::new(session);
            Ok(session)
        })
    }))
}

async fn resilient(manager: &SessionManager) -> TestResult<SessionId> {
    manager.set_keepalive(Some(KEEPALIVE));
    let resilience = Resilience {
        reconnect: reconnect()?,
        dialect: Some(Dialect::MySql),
        policy: RetryPolicy {
            probe_timeout: PROBE_TIMEOUT,
            first_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(400),
            attempts: 10,
            ..RetryPolicy::default()
        },
    };
    Ok(manager.open_resilient(resilience).await?)
}

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
    tokio::time::timeout(Duration::from_secs(20), wait).await?
}

/// The last event of `sql` on `session`.
async fn outcome(
    manager: &SessionManager,
    session: SessionId,
    sql: &str,
) -> TestResult<QueryEvent> {
    let mut handle = manager.execute(session, sql)?;
    let mut last = None;
    while let Some(event) = handle.next_event().await {
        last = Some(event);
    }
    Ok(last.ok_or("no events")?)
}

/// The single text value `sql` returns on `session`.
async fn text(
    manager: &SessionManager,
    texts: &Texts,
    session: SessionId,
    sql: &str,
) -> TestResult<String> {
    run(manager, session, sql).await?;
    Ok(texts.last().ok_or("no value")?)
}

async fn connection_id(
    manager: &SessionManager,
    texts: &Texts,
    session: SessionId,
) -> TestResult<String> {
    text(
        manager,
        texts,
        session,
        "SELECT CAST(CONNECTION_ID() AS CHAR)",
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn a_killed_idle_connection_is_replaced_with_its_database_restored() -> TestResult {
    let texts = Arc::new(Texts::default());
    let manager = SessionManager::new(texts.clone());
    let mut changes = manager.subscribe();
    let admin = open(&manager).await?;
    run(&manager, admin, "CREATE DATABASE IF NOT EXISTS m108_other").await?;
    let session = resilient(&manager).await?;
    run(&manager, session, "USE m108_other").await?;
    let before = connection_id(&manager, &texts, session).await?;
    run(&manager, admin, &format!("KILL {before}")).await?;
    let killed = Instant::now();
    reaches(&mut changes, session, SessionState::Reconnecting).await?;
    let detected = killed.elapsed();
    assert!(detected <= KEEPALIVE + PROBE_TIMEOUT, "{detected:?}");
    reaches(&mut changes, session, SessionState::Idle).await?;
    assert_ne!(before, connection_id(&manager, &texts, session).await?);
    let database = text(&manager, &texts, session, "SELECT DATABASE()").await?;
    assert_eq!(database, "m108_other");
    run(&manager, admin, "DROP DATABASE m108_other").await?;
    manager.close(session).await?;
    manager.close(admin).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn a_write_killed_while_running_fails_and_is_never_retried() -> TestResult {
    let texts = Arc::new(Texts::default());
    let manager = SessionManager::new(texts.clone());
    let mut changes = manager.subscribe();
    let admin = open(&manager).await?;
    run(&manager, admin, "DROP TABLE IF EXISTS m108_inflight").await?;
    run(&manager, admin, "CREATE TABLE m108_inflight (id int)").await?;
    let session = resilient(&manager).await?;
    let id = connection_id(&manager, &texts, session).await?;
    let insert = "INSERT INTO m108_inflight SELECT 1 FROM (SELECT SLEEP(2)) AS pause";
    let mut running = manager.execute(session, insert)?;
    assert!(matches!(
        running.next_event().await,
        Some(QueryEvent::Started { .. })
    ));
    run(&manager, admin, &format!("KILL {id}")).await?;
    let mut last = None;
    while let Some(event) = running.next_event().await {
        last = Some(event);
    }
    let Some(QueryEvent::Failed { error }) = last else {
        return Err("the insert must fail".into());
    };
    assert!(
        error.message.contains("may or may not have been applied"),
        "{}",
        error.message
    );
    reaches(&mut changes, session, SessionState::Idle).await?;
    let count = "SELECT CAST(COUNT(*) AS CHAR) FROM m108_inflight";
    assert_eq!(text(&manager, &texts, session, count).await?, "0");
    let done = outcome(&manager, admin, "DROP TABLE m108_inflight").await?;
    assert!(matches!(done, QueryEvent::Completed { .. }));
    manager.close(session).await?;
    manager.close(admin).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
async fn an_edit_commit_killed_mid_transaction_reports_a_rollback() -> TestResult {
    let texts = Arc::new(Texts::default());
    let manager = SessionManager::new(texts.clone());
    let admin = open(&manager).await?;
    run(&manager, admin, "DROP TABLE IF EXISTS m108_edits").await?;
    run(
        &manager,
        admin,
        "CREATE TABLE m108_edits (id int PRIMARY KEY) ENGINE=InnoDB",
    )
    .await?;
    let mut editor =
        openrows_driver_mysql::connect(&request()?, &mut ConnectTrace::default()).await?;
    let mut ids = Column::default();
    editor
        .execute("SELECT CAST(CONNECTION_ID() AS CHAR)", &mut ids)
        .await?;
    let id = ids.0.pop().ok_or("no id")?;
    let edits = [
        BoundSql {
            sql: "INSERT INTO m108_edits VALUES (1)".to_owned(),
            params: Vec::new(),
        },
        BoundSql {
            sql: format!("KILL {id}"),
            params: Vec::new(),
        },
    ];
    let failed = editor.commit_edits(&edits).await.err().ok_or("committed")?;
    assert!(matches!(failed, DriverError::Network(_)), "{failed}");
    assert!(
        failed.to_string().contains("rolled the transaction back"),
        "{failed}"
    );
    let count = "SELECT CAST(COUNT(*) AS CHAR) FROM m108_edits";
    assert_eq!(text(&manager, &texts, admin, count).await?, "0");
    run(&manager, admin, "DROP TABLE m108_edits").await?;
    manager.close(admin).await?;
    Ok(())
}
