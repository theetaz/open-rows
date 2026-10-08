// SPDX-License-Identifier: Apache-2.0
//! [`Rows`] consumers: Arrow batches into a [`ResultSink`] (user queries),
//! text rows (catalog and edit queries) and nothing (control statements).
use std::time::Instant;

use mysql_common::packets::Column;
use openrows_driver_api::{BatchPolicy, DriverError, ExecSummary, ResultSink, SinkFlow};

use crate::decode::BatchBuilder;
use crate::wire::Rows;

/// Streams every result set into a sink in policy-sized batches.
pub(crate) struct ArrowRows<'a> {
    sink: &'a mut dyn ResultSink,
    policy: BatchPolicy,
    batch: Option<BatchBuilder>,
    pub(crate) summary: ExecSummary,
    /// At least one result set was returned.
    pub(crate) had_rows: bool,
    first_row: Option<Instant>,
}

impl<'a> ArrowRows<'a> {
    pub(crate) fn new(sink: &'a mut dyn ResultSink, policy: BatchPolicy) -> Self {
        Self {
            sink,
            policy,
            batch: None,
            summary: ExecSummary::default(),
            had_rows: false,
            first_row: None,
        }
    }

    /// Hand the open batch to the sink; `false` when the sink stops.
    fn flush(&mut self) -> Result<bool, DriverError> {
        let Some(batch) = self.batch.as_mut() else {
            return Ok(true);
        };
        let batch = batch.finish()?;
        if self.summary.batches == 0 {
            self.summary.first_batch_latency = self.first_row.map(|start| start.elapsed());
        }
        self.summary.rows += batch.num_rows() as u64;
        self.summary.batches += 1;
        Ok(self.sink.push_batch(batch) == SinkFlow::Continue)
    }
}

impl Rows for ArrowRows<'_> {
    fn begin(&mut self, columns: &[Column]) -> Result<(), DriverError> {
        let batch = BatchBuilder::new(columns);
        self.sink.begin_result(batch.schema());
        self.batch = Some(batch);
        self.had_rows = true;
        Ok(())
    }

    fn row(&mut self, values: Vec<Option<Vec<u8>>>) -> Result<bool, DriverError> {
        self.first_row.get_or_insert_with(Instant::now);
        let Some(batch) = self.batch.as_mut() else {
            return Err(DriverError::Protocol("row outside a result set".to_owned()));
        };
        batch.append(&values)?;
        let width = values.len();
        let limit = self.policy.row_limit(width, self.summary.batches == 0);
        if self.policy.should_flush(batch.rows(), batch.bytes(), limit) {
            return self.flush();
        }
        Ok(true)
    }

    fn end(&mut self) -> Result<(), DriverError> {
        if self.batch.as_ref().is_some_and(|batch| batch.rows() > 0) {
            self.flush()?;
        }
        self.batch = None;
        Ok(())
    }
}

/// Collects every row as text (`None` is NULL).
#[derive(Default)]
pub(crate) struct TextRows(pub(crate) Vec<Vec<Option<String>>>);

impl Rows for TextRows {
    fn begin(&mut self, _columns: &[Column]) -> Result<(), DriverError> {
        Ok(())
    }

    fn row(&mut self, values: Vec<Option<Vec<u8>>>) -> Result<bool, DriverError> {
        self.0.push(
            values
                .into_iter()
                .map(|value| value.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()))
                .collect(),
        );
        Ok(true)
    }

    fn end(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
}

/// Drops rows (control statements such as `COMMIT`).
pub(crate) struct Discard;

impl Rows for Discard {
    fn begin(&mut self, _columns: &[Column]) -> Result<(), DriverError> {
        Ok(())
    }

    fn row(&mut self, _values: Vec<Option<Vec<u8>>>) -> Result<bool, DriverError> {
        Ok(false)
    }

    fn end(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
}
