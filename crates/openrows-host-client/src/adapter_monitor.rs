// SPDX-License-Identifier: Apache-2.0
//! `DuckDB` Monitor reads on the owning hosted connection. The session actor
//! runs them in queue order with user statements; an overrunning read is
//! cancelled on the host, and own statements feed a process-wide ledger.
use crate::{FileReservation, HostedConnection, duckdb_monitor};
use arrow_array::{Array, RecordBatch, StringArray};
use arrow_schema::SchemaRef;
use openrows_driver_api::{CancelHandle, DriverError, ExecSummary, ResultSink, SinkFlow};
use openrows_host_protocol::Control;
use openrows_monitor::{
    Descriptor, Execution, Ledger, MonitorReader, ReadFuture, Reading, SampleState, Source,
};
use std::sync::{Arc, atomic::Ordering};
use std::time::{Duration, Instant};

/// Rows kept from one diagnostic statement.
const ROWS: usize = 16;

/// The ledger shared by every session on this database file.
pub(crate) fn ledger(reservation: Option<&FileReservation>, pid: u32) -> Arc<Ledger> {
    let key = match reservation.and_then(FileReservation::canonical_path) {
        Some(path) => format!("duckdb:{}", path.display()),
        None => format!("duckdb-memory:{pid}"),
    };
    Ledger::shared(&key)
}

/// Record one finished user statement with its client wall time.
pub(crate) fn record(
    ledger: &Ledger,
    sql: &str,
    started: Instant,
    result: &Result<ExecSummary, DriverError>,
) {
    ledger.record(
        sql,
        Execution {
            elapsed_ms: started.elapsed().as_secs_f64() * 1_000.0,
            rows: result.as_ref().map_or(0, |summary| summary.rows),
            failed: result.is_err(),
            steps: None,
        },
    );
}

#[derive(Default)]
struct TextRows(Vec<Vec<Option<String>>>);
impl ResultSink for TextRows {
    fn begin_result(&mut self, _schema: SchemaRef) {}
    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        for row in 0..batch.num_rows() {
            if self.0.len() >= ROWS {
                return SinkFlow::Stop;
            }
            self.0.push(
                batch
                    .columns()
                    .iter()
                    .map(|array| text(array.as_ref(), row))
                    .collect(),
            );
        }
        SinkFlow::Continue
    }
}

fn text(array: &dyn Array, row: usize) -> Option<String> {
    let strings = array.as_any().downcast_ref::<StringArray>()?;
    (!strings.is_null(row)).then(|| strings.value(row).to_owned())
}

impl HostedConnection {
    /// Run internal diagnostic SQL; past `limit` it is cancelled on the host
    /// and the reply still drained, so the protocol stays in step.
    async fn diagnostic_rows(
        &mut self,
        sql: &str,
        limit: Duration,
    ) -> Result<Vec<Vec<Option<String>>>, DriverError> {
        let mut sink = TextRows::default();
        let request = self.begin(&Control::ExecuteBound {
            sql: sql.into(),
            params: Vec::new(),
        })?;
        let cancel = Arc::clone(&self.cancel);
        let watchdog = tokio::spawn(async move {
            tokio::time::sleep(limit).await;
            let _ = cancel.cancel().await;
        });
        let result = self.stream(request, &mut sink).await;
        self.cancel.active.store(0, Ordering::SeqCst);
        watchdog.abort();
        result.map(|_| sink.0)
    }

    async fn database(&mut self, deadline_ms: u32) -> Reading {
        let Some(ledger) = self.ledger.clone() else {
            return Reading::missing(duckdb_monitor::unavailable(Source::Database));
        };
        let limit = Duration::from_millis(u64::from(deadline_ms.clamp(4, 1_000)));
        match self.diagnostic_rows(duckdb_monitor::STATS, limit).await {
            Ok(rows) => duckdb_monitor::reading(
                rows.first().map(Vec::as_slice),
                ledger.totals(),
                ledger.epoch(),
            ),
            Err(error) => failure(&error),
        }
    }
}

fn failure(error: &DriverError) -> Reading {
    let state = match error {
        DriverError::Host(failure) => {
            let mut reading = Reading::missing(SampleState::Unavailable(format!(
                "The owning DuckDB {failure}"
            )));
            reading.terminal = true;
            return reading;
        }
        DriverError::Server { code, .. } if code == "57014" => SampleState::Gap,
        _ => SampleState::Unavailable("DuckDB diagnostics could not be read".into()),
    };
    Reading::missing(state)
}

impl MonitorReader for HostedConnection {
    fn discover(&mut self) -> ReadFuture<'_, Vec<Descriptor>> {
        Box::pin(async { duckdb_monitor::duckdb_monitor_descriptors() })
    }

    fn read(&mut self, source: Source, deadline_ms: u32) -> ReadFuture<'_, Reading> {
        Box::pin(async move {
            match (source, self.ledger.as_ref()) {
                (Source::Statements, Some(ledger)) => ledger.statements(),
                (Source::Database, _) => self.database(deadline_ms).await,
                (other, _) => Reading::missing(duckdb_monitor::unavailable(other)),
            }
        })
    }

    fn cancellation(&self) -> ReadFuture<'static, ()> {
        let cancel = Arc::clone(&self.cancel);
        Box::pin(async move {
            let _ = cancel.cancel().await;
        })
    }
}
