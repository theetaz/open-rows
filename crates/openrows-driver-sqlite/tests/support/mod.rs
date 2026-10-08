// SPDX-License-Identifier: Apache-2.0
//! Shared helpers: a temp database file and a collecting sink.
#![allow(dead_code, reason = "each test binary uses a subset")]

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::{DriverError, ExecSummary, ResultSink, Session, SinkFlow};
use openrows_driver_sqlite::{SqliteSession, open};

pub type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Collects everything; stops after `stop_after` batches when set.
#[derive(Default)]
pub struct Collect {
    pub schemas: Vec<SchemaRef>,
    pub batches: Vec<RecordBatch>,
    pub stop_after: Option<usize>,
}

impl ResultSink for Collect {
    fn begin_result(&mut self, schema: SchemaRef) {
        self.schemas.push(schema);
    }

    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        self.batches.push(batch);
        match self.stop_after {
            Some(limit) if self.batches.len() >= limit => SinkFlow::Stop,
            _ => SinkFlow::Continue,
        }
    }
}

impl Collect {
    pub fn rows(&self) -> usize {
        self.batches.iter().map(RecordBatch::num_rows).sum()
    }
}

/// A new database file in `dir`, created and filled by `setup`.
pub async fn database(dir: &tempfile::TempDir, setup: &str) -> Result<SqliteSession, DriverError> {
    let path = dir.path().join("test.sqlite");
    let mut session = open(&path, false, true)?;
    run(&mut session, setup).await?;
    Ok(session)
}

/// Run each statement of `script`, split with the `SQLite` rules.
pub async fn run(session: &mut SqliteSession, script: &str) -> Result<ExecSummary, DriverError> {
    let mut last = ExecSummary::default();
    for statement in openrows_sql::split(script, openrows_sql::Dialect::Sqlite) {
        last = session
            .execute(statement.text(script), &mut Collect::default())
            .await?;
    }
    Ok(last)
}

/// The SQLSTATE of a server error, or a description of anything else.
pub fn code<T>(result: Result<T, DriverError>) -> String {
    match result {
        Err(DriverError::Server { code, .. }) => code,
        Err(other) => format!("{other:?}"),
        Ok(_) => "ok".to_owned(),
    }
}
