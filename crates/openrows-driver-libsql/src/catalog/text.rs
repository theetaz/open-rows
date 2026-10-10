// SPDX-License-Identifier: Apache-2.0
//! Bounded internal catalog rows; a truncated catalog is refused, not partial.
use crate::LibsqlSession;
use arrow_array::{Array, Float64Array, Int64Array, RecordBatch, StringArray};
use arrow_schema::SchemaRef;
use openrows_driver_api::{DriverError, NoticeLevel, ResultSink, SinkFlow};

#[derive(Default)]
struct TextRows {
    rows: Vec<Vec<Option<String>>>,
    truncated: bool,
}
impl ResultSink for TextRows {
    fn begin_result(&mut self, _schema: SchemaRef) {}
    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        for row in 0..batch.num_rows() {
            self.rows.push(
                batch
                    .columns()
                    .iter()
                    .map(|array| text(array.as_ref(), row))
                    .collect(),
            );
        }
        SinkFlow::Continue
    }
    fn notice(&mut self, _level: NoticeLevel, _message: &str) {
        self.truncated = true;
    }
}
fn text(array: &dyn Array, row: usize) -> Option<String> {
    if array.is_null(row) {
        return None;
    }
    if let Some(array) = array.as_any().downcast_ref::<StringArray>() {
        return Some(array.value(row).into());
    }
    if let Some(array) = array.as_any().downcast_ref::<Int64Array>() {
        return Some(array.value(row).to_string());
    }
    array
        .as_any()
        .downcast_ref::<Float64Array>()
        .map(|array| array.value(row).to_string())
}
impl LibsqlSession {
    pub(crate) async fn text_query(
        &mut self,
        sql: &str,
        params: &[Option<String>],
    ) -> Result<Vec<Vec<Option<String>>>, DriverError> {
        let mut sink = TextRows::default();
        self.execute_values(
            sql,
            params
                .iter()
                .map(|value| crate::Value::text(value.as_deref()))
                .collect(),
            Vec::new(),
            &mut sink,
        )
        .await?;
        if sink.truncated {
            return Err(DriverError::Unsupported(
                "libSQL catalog or snapshot exceeded the result row cap; increase libsql_row_cap"
                    .into(),
            ));
        }
        Ok(sink.rows)
    }
}

pub(super) fn value(row: &[Option<String>], index: usize) -> Result<String, DriverError> {
    row.get(index)
        .and_then(Clone::clone)
        .ok_or_else(crate::error::protocol)
}
pub(super) fn integer(row: &[Option<String>], index: usize) -> Result<i64, DriverError> {
    value(row, index)?
        .parse()
        .map_err(|_| crate::error::protocol())
}
pub(super) fn optional(row: &[Option<String>], index: usize) -> Option<String> {
    row.get(index).and_then(Clone::clone)
}
pub(crate) fn qualified(schema: &str, name: &str) -> Result<String, DriverError> {
    openrows_sql::Dialect::Sqlite
        .qualified(schema, name)
        .map_err(|error| DriverError::Unsupported(error.to_string()))
}
