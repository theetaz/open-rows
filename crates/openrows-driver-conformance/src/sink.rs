// SPDX-License-Identifier: Apache-2.0
//! A recording sink and small execution helpers shared by the groups.

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::{Connection, DriverError, ExecSummary, ResultSink, SinkFlow};

use crate::Failure;
use crate::cell::text;

/// Records every result, batch and notice; stops after `stop_after` batches.
#[derive(Default)]
pub(crate) struct Collect {
    pub(crate) schemas: Vec<SchemaRef>,
    pub(crate) batches: Vec<RecordBatch>,
    pub(crate) stop_after: Option<usize>,
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
    /// Rows across every batch.
    pub(crate) fn rows(&self) -> usize {
        self.batches.iter().map(RecordBatch::num_rows).sum()
    }

    /// Column `column` of every row as text (`None` is NULL).
    pub(crate) fn column(&self, column: usize) -> Result<Vec<Option<String>>, Failure> {
        let mut values = Vec::with_capacity(self.rows());
        for batch in &self.batches {
            let array = batch
                .columns()
                .get(column)
                .ok_or_else(|| Failure::Contract(format!("result has no column {column}")))?;
            for row in 0..batch.num_rows() {
                values.push(text(array.as_ref(), row)?);
            }
        }
        Ok(values)
    }

    /// The only value of a one-row, one-column result.
    pub(crate) fn single(&self) -> Result<Option<String>, Failure> {
        let values = self.column(0)?;
        match values.as_slice() {
            [value] => Ok(value.clone()),
            other => Err(Failure::Contract(format!(
                "expected one row, got {}",
                other.len()
            ))),
        }
    }
}

/// Run `sql` and collect its rows.
pub(crate) async fn query(
    connection: &mut dyn Connection,
    sql: &str,
) -> Result<(Collect, ExecSummary), DriverError> {
    let mut sink = Collect::default();
    let summary = connection.execute(sql, &mut sink).await?;
    Ok((sink, summary))
}

/// Run each statement in order, discarding rows.
pub(crate) async fn run_all(
    connection: &mut dyn Connection,
    statements: &[String],
) -> Result<(), DriverError> {
    for sql in statements {
        query(connection, sql).await?;
    }
    Ok(())
}

/// The SQLSTATE of a `Server` error; anything else is a contract failure.
pub(crate) fn sqlstate<T>(result: Result<T, DriverError>, what: &str) -> Result<String, Failure> {
    match result {
        Err(DriverError::Server { code, .. }) => Ok(code),
        Err(other) => Err(Failure::Contract(format!(
            "{what}: expected a server error, got {other}"
        ))),
        Ok(_) => Err(Failure::Contract(format!("{what}: succeeded unexpectedly"))),
    }
}

/// `SELECT 1` answers `1`: the session is usable.
pub(crate) async fn ping(connection: &mut dyn Connection) -> Result<(), Failure> {
    let (sink, _) = query(connection, "SELECT 1").await?;
    let value = sink.single()?;
    crate::ensure(value.as_deref() == Some("1"), || {
        format!("SELECT 1 answered {value:?}")
    })
}
