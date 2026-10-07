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
}

impl QuerySink {
    pub(crate) fn new(events: UnboundedSender<QueryEvent>, store: Arc<dyn BatchSink>) -> Self {
        Self {
            events,
            store,
            open: None,
            results: 0,
            progress: Throttle::new(PROGRESS_INTERVAL),
        }
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
        open.rows += batch.num_rows() as u64;
        open.bytes += batch.get_array_memory_size() as u64;
        let flow = self.store.push(open.id, batch);
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
