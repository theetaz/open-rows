// SPDX-License-Identifier: Apache-2.0
//! Shared helpers for tests against a real server started by `just test-pg`.
use arrow_array::cast::AsArray;
use arrow_array::{ArrayRef, RecordBatch};
use arrow_schema::SchemaRef;
use openrows_driver_api::{DriverError, ExecSummary, NoticeLevel, ResultSink, Session, SinkFlow};
use openrows_driver_postgres::{PgConfig, PgSession};
use std::time::{Duration, Instant};

/// Records everything the driver hands over.
#[derive(Default)]
pub struct Collect {
    pub schema: Option<SchemaRef>,
    /// Number of `begin_result` calls.
    pub results: usize,
    pub notices: Vec<String>,
    pub batches: Vec<RecordBatch>,
    pub started: Option<Instant>,
    pub first_batch_after: Option<Duration>,
    pub stop_after_first: bool,
}

impl ResultSink for Collect {
    fn begin_result(&mut self, schema: SchemaRef) {
        self.schema = Some(schema);
        self.results += 1;
    }

    fn notice(&mut self, _level: NoticeLevel, message: &str) {
        self.notices.push(message.to_owned());
    }

    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        if self.batches.is_empty() {
            self.first_batch_after = self.started.map(|start| start.elapsed());
        }
        self.batches.push(batch);
        if self.stop_after_first {
            SinkFlow::Stop
        } else {
            SinkFlow::Continue
        }
    }
}

/// Test failures carry context instead of panicking inside helpers.
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Port and password of the throwaway container, exported by `scripts/test-pg.sh`.
pub fn config() -> TestResult<PgConfig> {
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

pub async fn connect() -> TestResult<PgSession> {
    Ok(PgSession::connect(&config()?).await?)
}

pub async fn run(
    session: &mut PgSession,
    sql: &str,
    mut sink: Collect,
) -> (Result<ExecSummary, DriverError>, Collect) {
    sink.started = Some(Instant::now());
    let result = session.execute(sql, &mut sink).await;
    (result, sink)
}

/// Execute and return the single result batch.
pub async fn single_batch(session: &mut PgSession, sql: &str) -> TestResult<RecordBatch> {
    let (result, sink) = run(session, sql, Collect::default()).await;
    result?;
    let [batch] = <[RecordBatch; 1]>::try_from(sink.batches)
        .map_err(|batches| format!("expected one batch, got {}", batches.len()))?;
    Ok(batch)
}

pub fn column(batch: &RecordBatch, name: &str) -> TestResult<ArrayRef> {
    Ok(batch
        .column_by_name(name)
        .ok_or_else(|| format!("no column {name}"))?
        .clone())
}

/// Field metadata value, e.g. `openrows.source_type` or `ARROW:extension:name`.
pub fn metadata(batch: &RecordBatch, name: &str, key: &str) -> TestResult<String> {
    let schema = batch.schema();
    let field = schema.field_with_name(name)?;
    Ok(field
        .metadata()
        .get(key)
        .ok_or_else(|| format!("{name} has no {key}"))?
        .clone())
}

/// Text values of rows 1.. (row 0 is the all-NULL row in the type fixtures).
pub fn strings(batch: &RecordBatch, name: &str) -> TestResult<Vec<String>> {
    let array = column(batch, name)?;
    let values = array.as_string_opt::<i32>().ok_or("not Utf8")?;
    Ok(values
        .iter()
        .skip(1)
        .map(|v| v.unwrap_or("NULL").to_owned())
        .collect())
}
