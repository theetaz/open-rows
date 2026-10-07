// SPDX-License-Identifier: Apache-2.0
//! Session actor against a real `PostgreSQL` server in a throwaway container.
//!
//! Ignored by `just test`; `just test-pg [version]` starts the container, runs
//! them with `--run-ignored only` and removes it (skipping when Docker is absent).
use arrow_array::{RecordBatch, StringArray};
use arrow_ipc::reader::StreamReader;
use arrow_schema::SchemaRef;
use openrows_core::script::{self, Scope, ScriptCanceller, ScriptEvent, ScriptOutcome};
use openrows_core::{BatchSink, QueryEvent, QueryHandle, SessionManager, SessionState};
use openrows_driver_api::{NoticeLevel, Session, SinkFlow};
use openrows_driver_postgres::{PgConfig, PgSession};
use openrows_results::{ResultStore, Window};
use openrows_types::{ResultId, SessionId};
use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Counts rows the store would keep.
#[derive(Default)]
struct CountRows(AtomicU64);

impl BatchSink for CountRows {
    fn begin(&self, _result: ResultId, _schema: SchemaRef) {}

    fn push(&self, _result: ResultId, batch: RecordBatch) -> SinkFlow {
        self.0.fetch_add(batch.num_rows() as u64, Ordering::Relaxed);
        SinkFlow::Continue
    }

    fn end(&self, _result: ResultId) {}
}

/// Port and password of the throwaway container, exported by `scripts/test-pg.sh`.
fn config() -> TestResult<PgConfig> {
    let variable = |name: &str| {
        std::env::var(name)
            .map_err(|_| format!("{name} is unset: run these ignored tests with `just test-pg`"))
    };
    Ok(PgConfig {
        host: "127.0.0.1".to_owned(),
        port: variable("OPENROWS_TEST_PG_PORT")?.parse()?,
        user: "postgres".to_owned(),
        password: Some(variable("OPENROWS_TEST_PG_PASSWORD")?),
        database: "postgres".to_owned(),
    })
}

async fn open(manager: &SessionManager) -> TestResult<SessionId> {
    let config = config()?;
    let id = manager
        .open(async move {
            let session: Box<dyn Session> = Box::new(PgSession::connect(&config).await?);
            Ok(session)
        })
        .await?;
    Ok(id)
}

async fn events(handle: &mut QueryHandle) -> TestResult<Vec<QueryEvent>> {
    let mut seen = Vec::new();
    loop {
        let event = tokio::time::timeout(Duration::from_secs(30), handle.next_event())
            .await?
            .ok_or("event stream ended without a terminal event")?;
        let terminal = event.is_terminal();
        seen.push(event);
        if terminal {
            return Ok(seen);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
async fn query_events_notices_and_cancel_on_postgres() -> TestResult {
    let store = Arc::new(CountRows::default());
    let manager = SessionManager::new(store.clone());
    let session = open(&manager).await?;

    let mut select = manager.execute(session, "SELECT generate_series(1, 100000) AS n")?;
    let seen = events(&mut select).await?;
    assert!(matches!(seen[0], QueryEvent::Started { .. }));
    assert!(matches!(seen[1], QueryEvent::ResultBegin { .. }));
    let end = &seen[seen.len() - 2];
    assert!(matches!(
        end,
        QueryEvent::ResultEnd {
            rows: 100_000,
            truncated: false,
            ..
        }
    ));
    assert!(matches!(seen.last(), Some(QueryEvent::Completed { .. })));
    assert_eq!(store.0.load(Ordering::Relaxed), 100_000);

    let mut warn = manager.execute(session, "DO $$ BEGIN RAISE WARNING 'careful'; END $$")?;
    let seen = events(&mut warn).await?;
    assert!(seen.iter().any(|event| matches!(
        event,
        QueryEvent::Notice { level: NoticeLevel::Warning, message } if message == "careful"
    )));

    let mut sleep = manager.execute(session, "SELECT pg_sleep(30)")?;
    assert!(matches!(
        sleep.next_event().await,
        Some(QueryEvent::Started { .. })
    ));
    tokio::time::sleep(Duration::from_millis(200)).await;
    let requested = Instant::now();
    sleep.canceller().cancel();
    assert!(matches!(
        events(&mut sleep).await?.pop(),
        Some(QueryEvent::Cancelled)
    ));
    assert!(
        requested.elapsed() < Duration::from_secs(2),
        "{:?}",
        requested.elapsed()
    );
    assert_eq!(manager.state(session), Some(SessionState::Idle));

    let mut after = manager.execute(session, "SELECT 1")?;
    assert!(matches!(
        events(&mut after).await?.pop(),
        Some(QueryEvent::Completed { .. })
    ));
    manager.close(session).await?;
    Ok(())
}

/// The `ResultEnd` result id of the first result with `rows` rows.
fn result_with(seen: &[ScriptEvent], rows: u64) -> TestResult<ResultId> {
    seen.iter()
        .find_map(|event| match &event.event {
            QueryEvent::ResultEnd {
                result, rows: n, ..
            } if *n == rows => Some(*result),
            _ => None,
        })
        .ok_or_else(|| format!("no {rows}-row result").into())
}

/// Labels (column 1) of an encoded window.
fn labels(bytes: Vec<u8>) -> TestResult<Vec<String>> {
    let mut out = Vec::new();
    for batch in StreamReader::try_new(Cursor::new(bytes), None)? {
        let batch = batch?;
        let column = batch
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or("label is not Utf8")?;
        out.extend(column.iter().flatten().map(str::to_owned));
    }
    Ok(out)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
async fn script_streams_into_the_result_store_and_serves_windows() -> TestResult {
    let store = Arc::new(ResultStore::default());
    let manager = SessionManager::new(store.clone());
    let session = open(&manager).await?;
    let sql = "SELECT n, 'row ' || n AS label FROM generate_series(1, 5000) AS n;\n\
               CREATE TEMP TABLE scratch (id int);\n\
               INSERT INTO scratch SELECT generate_series(1, 3);";
    let statements = script::statements(sql, Scope::All);
    assert_eq!(statements.len(), 3);
    let mut seen = Vec::new();
    let cancel = ScriptCanceller::default();
    let outcome = script::run_script(&manager, session, &statements, &cancel, |event| {
        seen.push(event);
    })
    .await;
    assert_eq!(outcome, ScriptOutcome::Completed);
    assert!(seen.iter().any(|event| event.statement == 2
        && matches!(event.event, QueryEvent::RowsAffected { count: 3 })));
    let result = result_with(&seen, 5000)?.to_string();
    assert_eq!(store.rows(&result), Some((5000, false)));
    let window = Window {
        row_start: 4990,
        row_count: 256,
        col_start: 0,
        col_count: 2,
    };
    let rows = labels(store.window(&result, window)?)?;
    assert_eq!(rows.len(), 10);
    assert_eq!(rows.first().map(String::as_str), Some("row 4991"));
    assert!(store.remove(&result));
    manager.close(session).await?;
    Ok(())
}
