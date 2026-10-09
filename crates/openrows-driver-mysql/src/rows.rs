// SPDX-License-Identifier: Apache-2.0
//! [`Rows`] consumers: Arrow batches into a [`ResultSink`] (user queries),
//! text rows (catalog and edit queries) and nothing (control statements).
//!
//! A result's schema is published with its first batch, so a column that
//! falls back to tagged text in that batch never changes a schema the sink
//! has seen; a fallback after it continues in a new result (docs/05 §12).
use std::sync::Arc;
use std::time::Instant;

use arrow_schema::SchemaRef;
use mysql_common::packets::Column;
use openrows_driver_api::{
    BatchPolicy, DriverError, ExecSummary, NoticeLevel, ResultSink, SinkFlow,
};

use crate::decode::BatchBuilder;
use crate::wire::Rows;

/// Explains why one result set arrives as two results.
const CONTINUED: &str = "a column held values outside its Arrow type (e.g. a zero date); the \
                         remaining rows continue in a new result with that column as text";

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
    fn begin(&mut self, columns: &[Column]) -> Result<(), DriverError> {
        self.batch = Some(BatchBuilder::new(columns));
        self.published = None;
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
        // A result without rows is published when it ends.
        if let Some(batch) = self.batch.take()
            && self.published.is_none()
        {
            self.publish(batch.schema());
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::RecordBatch;
    use arrow_schema::DataType;
    use mysql_common::constants::ColumnType;

    /// Records what the driver hands the sink.
    #[derive(Default)]
    struct Recorder {
        schemas: Vec<SchemaRef>,
        batches: Vec<RecordBatch>,
        notices: Vec<String>,
    }

    impl ResultSink for Recorder {
        fn begin_result(&mut self, schema: SchemaRef) {
            self.schemas.push(schema);
        }

        fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
            self.batches.push(batch);
            SinkFlow::Continue
        }

        fn notice(&mut self, _level: NoticeLevel, message: &str) {
            self.notices.push(message.to_owned());
        }
    }

    fn run(rows: &[&str]) -> Result<Recorder, DriverError> {
        let mut sink = Recorder::default();
        let policy = BatchPolicy {
            first_batch_rows: 2,
            target_cells: 2,
            max_bytes: 1 << 20,
        };
        let mut arrow = ArrowRows::new(&mut sink, policy);
        arrow.begin(&[Column::new(ColumnType::MYSQL_TYPE_DATE).with_name(b"d")])?;
        for text in rows {
            arrow.row(vec![Some(text.as_bytes().to_vec())])?;
        }
        arrow.end()?;
        Ok(sink)
    }

    #[test]
    fn a_fallback_inside_the_first_batch_keeps_one_result() -> Result<(), DriverError> {
        let sink = run(&["2024-01-01", "0000-00-00"])?;
        assert_eq!(sink.schemas.len(), 1);
        assert_eq!(sink.schemas[0].field(0).data_type(), &DataType::Utf8);
        assert!(sink.notices.is_empty());
        Ok(())
    }

    #[test]
    fn a_fallback_after_the_first_batch_continues_in_a_new_result() -> Result<(), DriverError> {
        let sink = run(&["2024-01-01", "2024-01-02", "0000-00-00"])?;
        let types: Vec<_> = sink
            .schemas
            .iter()
            .map(|schema| schema.field(0).data_type().clone())
            .collect();
        assert_eq!(types, [DataType::Date32, DataType::Utf8]);
        assert_eq!(sink.batches.len(), 2);
        assert_eq!(sink.notices, [CONTINUED]);
        Ok(())
    }

    #[test]
    fn an_empty_result_is_still_published() -> Result<(), DriverError> {
        let sink = run(&[])?;
        assert_eq!(sink.schemas.len(), 1);
        assert!(sink.batches.is_empty());
        Ok(())
    }
}
