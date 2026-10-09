// SPDX-License-Identifier: Apache-2.0
//! Bound, text-only catalog queries for object inspection.
use crate::session::PgSession;
use crate::simple::TextRow;
use arrow_array::{Array, RecordBatch, StringArray};
use arrow_schema::SchemaRef;
use openrows_driver_api::{DriverError, ResultSink, Session, SinkFlow};

#[derive(Default)]
struct TextSink {
    rows: Vec<TextRow>,
    invalid: bool,
}

impl ResultSink for TextSink {
    fn begin_result(&mut self, _schema: SchemaRef) {}

    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        let columns: Option<Vec<_>> = batch
            .columns()
            .iter()
            .map(|column| column.as_any().downcast_ref::<StringArray>())
            .collect();
        let Some(columns) = columns else {
            self.invalid = true;
            return SinkFlow::Stop;
        };
        self.rows.extend((0..batch.num_rows()).map(|index| {
            columns
                .iter()
                .map(|column| (!column.is_null(index)).then(|| column.value(index).to_owned()))
                .collect()
        }));
        SinkFlow::Continue
    }
}

/// Execute a catalog query whose output columns are explicitly text.
pub(super) async fn query(
    session: &mut PgSession,
    sql: &str,
    params: &[Option<String>],
) -> Result<Vec<TextRow>, DriverError> {
    let mut sink = TextSink::default();
    session.execute_bound(sql, params, &mut sink).await?;
    if sink.invalid {
        return Err(DriverError::Protocol(
            "Catalog query returned a non-text column".to_owned(),
        ));
    }
    Ok(sink.rows)
}

/// Names bound as text, never interpolated identifiers.
pub(super) fn bind(values: &[&str]) -> Vec<Option<String>> {
    values
        .iter()
        .map(|value| Some((*value).to_owned()))
        .collect()
}
