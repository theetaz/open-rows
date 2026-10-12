// SPDX-License-Identifier: Apache-2.0
//! Statement-scoped cancellation: no cancel request may stop a later statement.
use crate::support::{Collect, TestResult, connect, run};
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::{
    BoxFuture, CancelHandle, DriverError, ProcessId, ResultSink, Session, SinkFlow, admin,
};
use openrows_driver_postgres::PgSession;
use std::sync::Arc;
use std::time::{Duration, Instant};

const DEADLINE: Duration = Duration::from_secs(10);

/// Requests a cancel while its statement is being read, without polling it.
struct CaptureCancel {
    handle: Arc<dyn CancelHandle>,
    request: Option<BoxFuture<'static, Result<(), DriverError>>>,
}

impl ResultSink for CaptureCancel {
    fn begin_result(&mut self, _schema: SchemaRef) {
        self.request = Some(self.handle.cancel());
    }

    fn push_batch(&mut self, _batch: RecordBatch) -> SinkFlow {
        SinkFlow::Continue
    }
}

fn cancelled(result: &Result<impl Sized, DriverError>) -> bool {
    matches!(result, Err(DriverError::Server { code, .. }) if code == "57014")
}

/// Wait until the backend runs the sleep, not just its Parse/Describe.
async fn wait_for_sleep(observer: &mut PgSession, id: ProcessId) -> TestResult {
    let mut ticks = tokio::time::interval(Duration::from_millis(5));
    let deadline = Instant::now() + DEADLINE;
    while Instant::now() < deadline {
        ticks.tick().await;
        let sleeping = admin(observer)?
            .list_processes()
            .await?
            .iter()
            .any(|process| {
                process.id == id
                    && process.state.as_deref() == Some("active")
                    && process.wait.as_deref() == Some("Timeout: PgSleep")
            });
        if sleeping {
            return Ok(());
        }
    }
    Err(format!("process {id} never ran its sleep").into())
}

/// A request created during one statement and polled during the next is a no-op.
async fn stale_cancel(session: &mut PgSession, observer: &mut PgSession) -> TestResult {
    let id = admin(session)?.own_process().await?;
    let mut sink = CaptureCancel {
        handle: session.cancel_handle().ok_or("missing cancel handle")?,
        request: None,
    };
    session.execute("SELECT 1", &mut sink).await?;
    let request = sink.request.ok_or("no result captured")?;
    let execute = async {
        Ok::<_, Box<dyn std::error::Error>>(
            session
                .execute("SELECT pg_sleep(0.1)", &mut Collect::default())
                .await,
        )
    };
    let signal = async {
        wait_for_sleep(observer, id).await?;
        request.await?;
        Ok::<_, Box<dyn std::error::Error>>(())
    };
    let (next, ()) =
        tokio::time::timeout(DEADLINE, async { tokio::try_join!(execute, signal) }).await??;
    next?;
    Ok(())
}

/// Cancel an executing statement once; returns the request's latency.
async fn active_cancel(session: &mut PgSession, observer: &mut PgSession) -> TestResult<Duration> {
    let id = admin(session)?.own_process().await?;
    let handle = session.cancel_handle().ok_or("missing cancel handle")?;
    let execute = async {
        Ok::<_, Box<dyn std::error::Error>>(
            session
                .execute("SELECT pg_sleep(60)", &mut Collect::default())
                .await,
        )
    };
    let signal = async {
        wait_for_sleep(observer, id).await?;
        let requested = Instant::now();
        handle.cancel().await?;
        Ok::<_, Box<dyn std::error::Error>>(requested.elapsed())
    };
    let (result, latency) =
        tokio::time::timeout(DEADLINE, async { tokio::try_join!(execute, signal) }).await??;
    if !cancelled(&result) {
        return Err(format!("executing statement not cancelled: {result:?}").into());
    }
    run(session, "SELECT 42", Collect::default()).await.0?;
    Ok(latency)
}

/// A cancel racing a short statement's completion, detached like the session
/// actor's; true when it stopped that statement. The next statement must run.
async fn racing_cancel(session: &mut PgSession, offset: Duration) -> TestResult<bool> {
    let handle = session.cancel_handle().ok_or("missing cancel handle")?;
    let requested = tokio::spawn(async move {
        tokio::time::sleep(offset).await;
        drop(tokio::spawn(handle.cancel()));
    });
    let result = run(session, "SELECT pg_sleep(0.01)", Collect::default())
        .await
        .0;
    requested.await?;
    let stopped = cancelled(&result);
    if !stopped {
        result?;
    }
    run(session, "SELECT 42", Collect::default()).await.0?;
    Ok(stopped)
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn delayed_cancel_future_cannot_cancel_next_statement() -> TestResult {
    let mut session = connect().await?;
    let mut observer = connect().await?;
    stale_cancel(&mut session, &mut observer).await
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn single_cancel_of_executing_statement_leaves_session_usable() -> TestResult {
    let mut session = connect().await?;
    let mut observer = connect().await?;
    active_cancel(&mut session, &mut observer).await?;
    Ok(())
}

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn cancel_racing_completion_never_stops_next_statement() -> TestResult {
    let mut session = connect().await?;
    for offset in 0..=20 {
        racing_cancel(&mut session, Duration::from_millis(offset)).await?;
    }
    Ok(())
}

#[tokio::test]
#[ignore = "opt-in: OPENROWS_PG_CANCEL_STRESS=500 just test-pg"]
async fn cancel_race_stress() -> TestResult {
    let Ok(iterations) = std::env::var("OPENROWS_PG_CANCEL_STRESS") else {
        return Ok(());
    };
    let iterations: u64 = iterations.parse()?;
    if iterations < 500 {
        return Err("cancel stress requires at least 500 iterations".into());
    }
    let mut session = connect().await?;
    let mut observer = connect().await?;
    let mut latencies = Vec::new();
    let mut stopped = 0;
    for iteration in 0..iterations {
        latencies.push(active_cancel(&mut session, &mut observer).await?);
        stale_cancel(&mut session, &mut observer).await?;
        let offset = Duration::from_micros(iteration % 21 * 1_000);
        stopped += u64::from(racing_cancel(&mut session, offset).await?);
    }
    latencies.sort_unstable();
    report(iterations, stopped, &latencies);
    Box::new(session).close().await?;
    Box::new(observer).close().await?;
    Ok(())
}

#[expect(
    clippy::print_stderr,
    reason = "opt-in cancellation stress and latency evidence"
)]
fn report(iterations: u64, stopped: u64, latencies: &[Duration]) {
    let at = |percent: usize| latencies.get(latencies.len() * percent / 100);
    eprintln!(
        "cancel stress: {iterations} executing + {iterations} delayed-future + {iterations} racing \
         ({stopped} stopped their own statement) iterations; 0 misdirected cancels; \
         latency p50={:?} p95={:?} max={:?}",
        at(50),
        at(95),
        latencies.last(),
    );
}
