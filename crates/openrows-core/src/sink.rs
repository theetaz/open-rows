// SPDX-License-Identifier: Apache-2.0
//! Bridges driver output to the result-store boundary and the query event stream.
use crate::event::QueryEvent;
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::{ExecSummary, NoticeLevel, ResultSink, SinkFlow};
use openrows_types::ResultId;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedSender;

/// Minimum spacing of `Progress` events per query (≤ 10 Hz, docs/06 §4).
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// Where result batches go; the production result store implements this (D-07).
///
/// Called from session actors, so implementations must be cheap and must not block.
pub trait BatchSink: Send + Sync {
    /// A new result set with `schema` starts.
    fn begin(&self, result: ResultId, schema: SchemaRef);
    /// Append a batch; `SinkFlow::Stop` ends the stream early (row cap, memory).
    fn push(&self, result: ResultId, batch: RecordBatch) -> SinkFlow;
    /// The result set is complete; no more batches follow.
    fn end(&self, result: ResultId);
}

/// Allows at most one event per `interval`, the first immediately.
pub(crate) struct Throttle {
    interval: Duration,
    last: Option<Instant>,
}

impl Throttle {
    pub(crate) fn new(interval: Duration) -> Self {
        Self {
            interval,
            last: None,
        }
    }

    pub(crate) fn ready(&mut self, now: Instant) -> bool {
        let due = self
            .last
            .is_none_or(|last| now.saturating_duration_since(last) >= self.interval);
        if due {
            self.last = Some(now);
        }
        due
    }
}

struct OpenResult {
    id: ResultId,
    rows: u64,
    bytes: u64,
    truncated: bool,
}

/// One query's driver sink: forwards batches and emits result events.
pub(crate) struct QuerySink {
    events: UnboundedSender<QueryEvent>,
    store: Arc<dyn BatchSink>,
    open: Option<OpenResult>,
    results: usize,
    progress: Throttle,
    /// Rows kept per result set before the stream stops as truncated.
    max_rows: u64,
}

impl QuerySink {
    pub(crate) fn new(events: UnboundedSender<QueryEvent>, store: Arc<dyn BatchSink>) -> Self {
        Self {
            events,
            store,
            open: None,
            results: 0,
            progress: Throttle::new(PROGRESS_INTERVAL),
            max_rows: u64::MAX,
        }
    }

    /// Keep at most `max_rows` rows per result set (`None`: no per-run cap).
    pub(crate) fn with_max_rows(mut self, max_rows: Option<usize>) -> Self {
        self.max_rows = max_rows.map_or(u64::MAX, |rows| rows as u64);
        self
    }

    fn emit(&self, event: QueryEvent) {
        // A dropped receiver means nobody is listening; the query still runs to completion.
        let _ = self.events.send(event);
    }

    /// Close any open result set.
    pub(crate) fn end_result(&mut self) {
        let Some(open) = self.open.take() else {
            return;
        };
        self.store.end(open.id);
        self.emit(QueryEvent::ResultEnd {
            result: open.id,
            rows: open.rows,
            truncated: open.truncated,
        });
    }

    /// Close the result and report the row count of row-less statements.
    pub(crate) fn finish(&mut self, summary: &ExecSummary) {
        self.end_result();
        if self.results == 0
            && let Some(count) = affected_rows(&summary.command_tag)
        {
            self.emit(QueryEvent::RowsAffected { count });
        }
    }
}

impl ResultSink for QuerySink {
    fn begin_result(&mut self, schema: SchemaRef) {
        self.end_result();
        let id = ResultId::new();
        self.results += 1;
        self.store.begin(id, schema.clone());
        self.emit(QueryEvent::ResultBegin { result: id, schema });
        self.open = Some(OpenResult {
            id,
            rows: 0,
            bytes: 0,
            truncated: false,
        });
    }

    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        let Some(open) = self.open.as_mut() else {
            return SinkFlow::Stop;
        };
        let (batch, capped) = cap(batch, self.max_rows.saturating_sub(open.rows));
        open.rows += batch.num_rows() as u64;
        open.bytes += batch.get_array_memory_size() as u64;
        let stored = self.store.push(open.id, batch);
        let flow = if capped { SinkFlow::Stop } else { stored };
        open.truncated |= flow == SinkFlow::Stop;
        let progress = QueryEvent::Progress {
            result: open.id,
            rows: open.rows,
            bytes: open.bytes,
        };
        if self.progress.ready(Instant::now()) {
            self.emit(progress);
        }
        flow
    }

    fn notice(&mut self, level: NoticeLevel, message: &str) {
        self.emit(QueryEvent::Notice {
            level,
            message: message.to_owned(),
        });
    }
}

/// The part of `batch` that fits in `room` rows, and whether rows were cut.
fn cap(batch: RecordBatch, room: u64) -> (RecordBatch, bool) {
    let room = usize::try_from(room).unwrap_or(usize::MAX);
    if batch.num_rows() > room {
        (batch.slice(0, room), true)
    } else {
        (batch, false)
    }
}

/// Row count from a command tag such as `UPDATE 3` or `INSERT 0 5`.
fn affected_rows(tag: &str) -> Option<u64> {
    let mut words = tag.split_ascii_whitespace();
    let verb = words.next()?;
    let counted = ["INSERT", "UPDATE", "DELETE", "MERGE", "COPY"];
    if !counted.contains(&verb) {
        return None;
    }
    words.next_back()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throttle_allows_ten_events_per_second() {
        let mut throttle = Throttle::new(PROGRESS_INTERVAL);
        let start = Instant::now();
        let allowed = (0..1_000u64)
            .filter(|ms| throttle.ready(start + Duration::from_millis(*ms)))
            .count();
        assert_eq!(allowed, 10);
    }

    /// Push two 2-row batches through a sink capped at `max_rows`; returns the
    /// flows and the `ResultEnd` rows and truncation.
    fn capped_run(max_rows: usize) -> Result<(Vec<SinkFlow>, u64, bool), arrow_schema::ArrowError> {
        let store = Arc::new(openrows_results::ResultStore::default());
        let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
        let mut sink = QuerySink::new(events, store).with_max_rows(Some(max_rows));
        let batch = openrows_results::synthetic::batch(2, 2, 42)?;
        sink.begin_result(batch.schema());
        let flows = vec![sink.push_batch(batch.clone()), sink.push_batch(batch)];
        sink.end_result();
        let mut end = (0, false);
        while let Ok(event) = received.try_recv() {
            if let QueryEvent::ResultEnd {
                rows, truncated, ..
            } = event
            {
                end = (rows, truncated);
            }
        }
        Ok((flows, end.0, end.1))
    }

    #[test]
    fn a_run_row_cap_cuts_the_result_and_marks_it_truncated() -> Result<(), arrow_schema::ArrowError>
    {
        let (flows, rows, truncated) = capped_run(3)?;
        assert_eq!(flows, [SinkFlow::Continue, SinkFlow::Stop]);
        assert_eq!((rows, truncated), (3, true));
        let (flows, rows, truncated) = capped_run(4)?;
        assert_eq!(flows, [SinkFlow::Continue, SinkFlow::Continue]);
        assert_eq!((rows, truncated), (4, false));
        Ok(())
    }

    #[test]
    fn affected_rows_reads_dml_tags_only() {
        assert_eq!(affected_rows("UPDATE 3"), Some(3));
        assert_eq!(affected_rows("INSERT 0 5"), Some(5));
        assert_eq!(affected_rows("DELETE 0"), Some(0));
        assert_eq!(affected_rows("CREATE TABLE"), None);
        assert_eq!(affected_rows("SELECT 4"), None);
        assert_eq!(affected_rows(""), None);
    }
}
