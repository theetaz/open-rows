// SPDX-License-Identifier: Apache-2.0
//! [`Rows`] consumers: Arrow batches into a [`ResultSink`] (user queries)
//! and text rows (catalog, editor and admin queries).
//!
//! A result's schema is published with its first batch, so a column that
//! falls back to tagged text in that batch never changes a schema the sink
//! has seen; a fallback after it continues in a new result (docs/05 §12).
//! Every result set of a batch is a result of its own.

use std::sync::Arc;
use std::time::Instant;

use arrow_schema::SchemaRef;
use openrows_driver_api::{
    BatchPolicy, DriverError, ExecSummary, NoticeLevel, ResultSink, SinkFlow,
};

use crate::decode::builder::BatchBuilder;
use crate::decode::{Kind, kind, text};
use crate::error::server_error;
use crate::reply::{Count, Rows};
use crate::tds::token::{Column, Message, RowValues};

/// Explains why one result set arrives as two results.
const CONTINUED: &str = "a column held values outside its Arrow type (a datetime2 outside \
                         1677-2262); the remaining rows continue in a new result with that column \
                         as text";

/// Streams every result set into a sink in policy-sized batches.
pub(crate) struct ArrowRows<'a> {
    sink: &'a mut dyn ResultSink,
    policy: BatchPolicy,
    batch: Option<BatchBuilder>,
    /// The schema the sink last saw for the current result set.
    published: Option<SchemaRef>,
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
            published: None,
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
        self.publish(batch.schema());
        if self.summary.batches == 0 {
            self.summary.first_batch_latency = self.first_row.map(|start| start.elapsed());
        }
        self.summary.rows += batch.num_rows() as u64;
        self.summary.batches += 1;
        Ok(self.sink.push_batch(batch) == SinkFlow::Continue)
    }

    /// Start the result on first use and again whenever its schema changed.
    fn publish(&mut self, schema: SchemaRef) {
        match &self.published {
            Some(published) if Arc::ptr_eq(published, &schema) => return,
            Some(_) => self.sink.notice(NoticeLevel::Warning, CONTINUED),
            None => {}
        }
        self.sink.begin_result(Arc::clone(&schema));
        self.published = Some(schema);
    }
}

impl Rows for ArrowRows<'_> {
    fn begin(&mut self, columns: &Arc<[Column]>) -> Result<(), DriverError> {
        self.batch = Some(BatchBuilder::new(columns));
        self.published = None;
        self.had_rows = true;
        Ok(())
    }

    fn row(&mut self, values: RowValues<'_>) -> Result<bool, DriverError> {
        self.first_row.get_or_insert_with(Instant::now);
        let Some(batch) = self.batch.as_mut() else {
            return Err(DriverError::Protocol("row outside a result set".to_owned()));
        };
        batch.append(&values)?;
        let limit = self
            .policy
            .row_limit(values.len(), self.summary.batches == 0);
        if self.policy.should_flush(batch.rows(), batch.bytes(), limit) {
            return self.flush();
        }
        Ok(true)
    }

    fn end(&mut self) -> Result<(), DriverError> {
        if self.batch.as_ref().is_some_and(|batch| batch.rows() > 0) {
            self.flush()?;
        }
        // A result without rows is published when it ends.
        if let Some(batch) = self.batch.take()
            && self.published.is_none()
        {
            self.publish(batch.schema());
        }
        Ok(())
    }

    /// `PRINT` output (message number 0) is informational; other messages
    /// (`RAISERROR` with a low severity, ANSI warnings) and errors after
    /// the first warn. The server reports severities up to 10 as class 0,
    /// so the number, not the class, tells them apart.
    fn notice(&mut self, message: &Message) {
        let level = if message.error || message.number != 0 {
            NoticeLevel::Warning
        } else {
            NoticeLevel::Info
        };
        if message.error {
            self.sink.notice(level, &server_error(message).to_string());
        } else {
            self.sink.notice(level, &message.text);
        }
    }

    /// `(n rows affected)` for statements without a result set, as SQL
    /// Server's own tools print it.
    fn counted(&mut self, count: Count) {
        if !count.result {
            let noun = if count.rows == 1 { "row" } else { "rows" };
            let text = format!("({} {noun} affected)", count.rows);
            self.sink.notice(NoticeLevel::Info, &text);
        }
    }
}

/// Collects every row as text (`None` is NULL) in the server's text forms.
#[derive(Default)]
pub(crate) struct TextRows {
    kinds: Vec<Kind>,
    pub(crate) rows: Vec<Vec<Option<String>>>,
}

impl Rows for TextRows {
    fn begin(&mut self, columns: &Arc<[Column]>) -> Result<(), DriverError> {
        self.kinds = columns
            .iter()
            .map(|column| kind(&column.info.sql))
            .collect();
        Ok(())
    }

    fn row(&mut self, values: RowValues<'_>) -> Result<bool, DriverError> {
        let row = values
            .iter()
            .zip(&self.kinds)
            .enumerate()
            .map(|(column, (value, kind))| {
                value
                    .as_deref()
                    .map(|bytes| text::render(*kind, bytes))
                    .transpose()
                    .map_err(|reason| DriverError::Decode {
                        column,
                        reason: reason.to_string(),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.rows.push(row);
        Ok(true)
    }

    fn end(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
