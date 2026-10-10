// SPDX-License-Identifier: Apache-2.0
//! Session actor against a real `PostgreSQL` server in a throwaway container.
//!
//! Ignored by `just test`; `just test-pg [version]` starts the container, runs
//! them with `--run-ignored only` and removes it (skipping when Docker is absent).
#[path = "postgres/cancellation.rs"]
mod cancellation;
#[path = "postgres/explain.rs"]
mod explain;
#[path = "postgres/export.rs"]
mod export;
#[path = "postgres/limits.rs"]
mod limits;
#[path = "postgres/resilience.rs"]
mod resilience;
#[path = "postgres/script_file.rs"]
mod script_file;
#[path = "postgres/table_edits.rs"]
mod table_edits;
#[path = "postgres/table_filters.rs"]
mod table_filters;
#[path = "postgres/table_links.rs"]
mod table_links;
#[path = "postgres/table_view.rs"]
mod table_view;

use arrow_array::{RecordBatch, StringArray};
use arrow_ipc::reader::StreamReader;
use arrow_schema::SchemaRef;
use openrows_core::script::{self, Scope, Script, ScriptCanceller, ScriptEvent, ScriptOutcome};
use openrows_core::{BatchSink, QueryEvent, QueryHandle, SessionManager, SessionState};
use openrows_driver_api::{NoticeLevel, Session, SinkFlow};
use openrows_driver_postgres::{PgConfig, PgSession};
use openrows_results::{ResultStore, Window};
use openrows_sql::select::BoundSql;
use openrows_sql::{Dialect, ParamBinder, ParamStyle};
use openrows_types::{ResultId, SessionId};
use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

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
    open_on(manager, "postgres").await
}

/// A session on `database`: `PostgreSQL` cannot switch databases on a
/// connection, so the app opens one session per opened database.
async fn open_on(manager: &SessionManager, database: &str) -> TestResult<SessionId> {
    let config = PgConfig {
        database: database.to_owned(),
        ..config()?
    };
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

    cancellation::cancel_running_query(&manager, session).await?;
    assert_eq!(manager.state(session), Some(SessionState::Idle));

    let mut after = manager.execute(session, "SELECT 1")?;
    assert!(matches!(
        events(&mut after).await?.pop(),
        Some(QueryEvent::Completed { .. })
    ));
    manager.close(session).await?;
    Ok(())
}

/// Statement texts without parameters.
fn plain(statements: Vec<String>) -> Vec<BoundSql> {
    statements
        .into_iter()
        .map(|sql| BoundSql {
            sql,
            params: Vec::new(),
        })
        .collect()
}

/// An uncapped script of `statements` that stops at the first error.
fn script(statements: &[BoundSql]) -> Script<'_> {
    Script {
        statements,
        max_rows: None,
        stop_on_error: true,
        sink: None,
    }
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
    let statements = plain(script::statements(sql, Scope::All, Dialect::Postgres));
    assert_eq!(statements.len(), 3);
    let mut seen = Vec::new();
    let cancel = ScriptCanceller::default();
    let outcome = script::run_script(&manager, session, script(&statements), &cancel, |event| {
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
async fn editor_parameters_bind_and_errors_can_continue() -> TestResult {
    let store = Arc::new(ResultStore::default());
    let manager = SessionManager::new(store.clone());
    let session = open(&manager).await?;
    let texts = ["SELECT 1/0", "SELECT :n::int AS n, :label AS label"];
    let mut binder = ParamBinder::new(Dialect::Postgres, ParamStyle::Named);
    let mut statements = Vec::new();
    for sql in texts {
        let slots = binder.slots(sql);
        let value = |name: &str| Some(Some(if name == ":n" { "7" } else { "it's" }.to_owned()));
        statements.push(binder.bind(sql, &slots, value)?);
    }
    let mut seen = Vec::new();
    let run = Script {
        stop_on_error: false,
        ..script(&statements)
    };
    let cancel = ScriptCanceller::default();
    let outcome =
        script::run_script(&manager, session, run, &cancel, |event| seen.push(event)).await;
    assert_eq!(outcome, ScriptOutcome::Failed);
    assert!(
        seen.iter()
            .any(|event| event.statement == 0 && matches!(event.event, QueryEvent::Failed { .. }))
    );
    let result = result_with(&seen, 1)?.to_string();
    let window = Window {
        row_start: 0,
        row_count: 1,
        col_start: 0,
        col_count: 2,
    };
    assert_eq!(labels(store.window(&result, window)?)?, ["it's"]);
    manager.close(session).await?;
    Ok(())
}

/// Run `sql` as one script on `session` and return the `label` column of its
/// single-row result.
async fn single_label(
    manager: &SessionManager,
    store: &ResultStore,
    session: SessionId,
    sql: &str,
) -> TestResult<Vec<String>> {
    let mut seen = Vec::new();
    let statements = plain(script::statements(sql, Scope::All, Dialect::Postgres));
    let cancel = ScriptCanceller::default();
    let outcome = script::run_script(manager, session, script(&statements), &cancel, |event| {
        seen.push(event);
    })
    .await;
    assert_eq!(outcome, ScriptOutcome::Completed);
    let result = result_with(&seen, 1)?.to_string();
    let window = Window {
        row_start: 0,
        row_count: 1,
        col_start: 0,
        col_count: 2,
    };
    labels(store.window(&result, window)?)
}

/// Run one statement on `session` and expect it to complete.
async fn completes(manager: &SessionManager, session: SessionId, sql: &str) -> TestResult {
    let mut handle = manager.execute(session, sql)?;
    assert!(matches!(
        events(&mut handle).await?.pop(),
        Some(QueryEvent::Completed { .. })
    ));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs PostgreSQL; run with `just test-pg`"]
async fn two_databases_of_one_connection_run_side_by_side() -> TestResult {
    let store = Arc::new(ResultStore::default());
    let manager = SessionManager::new(store.clone());
    let home = open(&manager).await?;
    completes(&manager, home, "CREATE DATABASE openrows_second").await?;
    let second = open_on(&manager, "openrows_second").await?;
    let sql = "SELECT 1 AS n, current_database()::text AS label";
    let (first, other) = tokio::join!(
        single_label(&manager, &store, home, sql),
        single_label(&manager, &store, second, sql)
    );
    assert_eq!(first?, ["postgres"]);
    assert_eq!(other?, ["openrows_second"]);
    manager.close(second).await?;
    assert_eq!(manager.state(home), Some(SessionState::Idle));
    completes(&manager, home, "DROP DATABASE openrows_second").await?;
    manager.close(home).await?;
    Ok(())
}
