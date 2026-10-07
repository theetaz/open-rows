// SPDX-License-Identifier: Apache-2.0
//! Session actor against a real `PostgreSQL` server in a throwaway container.
//!
//! Ignored by `just test`; `just test-pg [version]` starts the container, runs
//! them with `--run-ignored only` and removes it (skipping when Docker is absent).
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_core::{BatchSink, QueryEvent, QueryHandle, SessionManager, SessionState};
use openrows_driver_api::{NoticeLevel, Session, SinkFlow};
use openrows_driver_postgres::{PgConfig, PgSession};
use openrows_types::{ResultId, SessionId};
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
