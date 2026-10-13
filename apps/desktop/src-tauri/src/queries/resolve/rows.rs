// SPDX-License-Identifier: Apache-2.0
//! Bounded text-only sink for the fixed source resolution queries.
use arrow_array::{Array, RecordBatch, StringArray};
use arrow_schema::SchemaRef;
use openrows_driver_api::{Connection, DriverError, ResultSink, SinkFlow};

#[derive(Default)]
struct Rows {
    rows: Vec<[String; 4]>,
    invalid: bool,
}
impl ResultSink for Rows {
    fn begin_result(&mut self, _schema: SchemaRef) {}

    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        let columns: Option<Vec<_>> = batch
            .columns()
            .iter()
            .map(|column| column.as_any().downcast_ref::<StringArray>())
            .collect();
        let Some(columns) = columns.filter(|columns| columns.len() == 4) else {
            self.invalid = true;
            return SinkFlow::Stop;
        };
        if batch.num_rows() + self.rows.len() > 2 || columns.iter().any(|c| c.null_count() > 0) {
            self.invalid = true;
            return SinkFlow::Stop;
        }
        for index in 0..batch.num_rows() {
            self.rows.push(std::array::from_fn(|column| {
                columns[column].value(index).into()
            }));
        }
        SinkFlow::Continue
    }
}

pub(super) async fn query(
    connection: &mut dyn Connection,
    sql: &str,
    name: &str,
) -> Result<Vec<[String; 4]>, DriverError> {
    let mut sink = Rows::default();
    connection
        .execute_bound(sql, &[Some(name.into())], &mut sink)
        .await?;
    if sink.invalid {
        return Err(DriverError::Protocol(
            "Invalid source resolution metadata".into(),
        ));
    }
    Ok(sink.rows)
}
