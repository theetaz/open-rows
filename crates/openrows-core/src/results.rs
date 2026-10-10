// SPDX-License-Identifier: Apache-2.0
//! The production [`BatchSink`]: session batches stream into the result store.
use crate::BatchSink;
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::{BoxFuture, SinkAdmission, SinkFlow};
use openrows_results::{Append, ResultStore};
use openrows_types::ResultId;

impl BatchSink for ResultStore {
    fn begin(&self, result: ResultId, schema: SchemaRef) {
        ResultStore::begin(self, &result.to_string(), schema);
    }

    fn push(&self, result: ResultId, batch: RecordBatch) -> SinkFlow {
        match self.append(&result.to_string(), batch) {
            Append::Continue => SinkFlow::Continue,
            Append::Full => SinkFlow::Stop,
        }
    }

    fn admission(&self, result: ResultId) -> SinkAdmission {
        admission(self.admission(&result.to_string()))
    }
    fn resume(&self, result: ResultId) -> BoxFuture<'_, SinkAdmission> {
        Box::pin(async move { admission(self.resume(&result.to_string()).await) })
    }
    fn stopped(&self, result: ResultId) {
        self.mark_truncated(&result.to_string());
    }
    /// Streamed results are readable as they grow, so completion needs no bookkeeping.
    fn end(&self, _result: ResultId) {}
}

fn admission(value: openrows_results::Admission) -> SinkAdmission {
    match value {
        openrows_results::Admission::Ready => SinkAdmission::Ready,
        openrows_results::Admission::Pause => SinkAdmission::Pause,
        openrows_results::Admission::Stop => SinkAdmission::Stop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openrows_results::{Limits, Window};

    #[test]
    fn store_sink_streams_and_stops_at_the_row_cap() -> Result<(), Box<dyn std::error::Error>> {
        let store = ResultStore::with_limits(Limits {
            max_rows: 3,
            ..Limits::default()
        });
        let batch = openrows_results::synthetic::batch(2, 2, 42)?;
        let id = ResultId::new();
        let sink: &dyn BatchSink = &store;
        sink.begin(id, batch.schema());
        assert_eq!(sink.push(id, batch.clone()), SinkFlow::Continue);
        assert_eq!(sink.push(id, batch), SinkFlow::Stop);
        sink.end(id);
        assert_eq!(store.rows(&id.to_string()), Some((3, true)));
        let window = Window {
            row_start: 0,
            row_count: 10,
            col_start: 0,
            col_count: 2,
        };
        assert!(!store.window(&id.to_string(), window)?.is_empty());
        assert_eq!(sink.push(ResultId::new(), store_batch()?), SinkFlow::Stop);
        Ok(())
    }

    fn store_batch() -> Result<RecordBatch, arrow_schema::ArrowError> {
        openrows_results::synthetic::batch(1, 2, 42)
    }
}
