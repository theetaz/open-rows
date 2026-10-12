// SPDX-License-Identifier: Apache-2.0
//! Own-request diagnostics over Hrana 3 (ADR-0011): client wall time, rows
//! and outcomes of the app's own statements plus a `SELECT 1` round trip.
//! Hrana is not a monitoring API, so server CPU, memory, storage, waits and
//! other clients' queries are explicitly unavailable, never estimated.
use std::time::{Duration, Instant};

use openrows_driver_api::{CancelHandle, DriverError, ExecSummary};
use openrows_monitor::{
    Column, Descriptor, Execution, Ledger, MetricKind, MonitorReader, ReadFuture, Reading,
    SampleState, Scope, Source, Totals,
};
use url::Url;

use crate::LibsqlSession;

/// The ADR-0011 boundary shown beside every remote libSQL panel.
const OWN: &str = "Own OpenRows requests only — server-wide metrics unavailable over Hrana.";

/// Ledger identity of a remote database: origin and path, never credentials.
pub(crate) fn ledger(url: &Url) -> std::sync::Arc<Ledger> {
    Ledger::shared(&format!(
        "libsql:{}{}",
        url.origin().ascii_serialization(),
        url.path()
    ))
}

/// Record one user statement's client-side outcome.
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

impl MonitorReader for LibsqlSession {
    fn discover(&mut self) -> ReadFuture<'_, Vec<Descriptor>> {
        Box::pin(async move {
            let probe = round_trip(self, 2_000).await;
            Source::ALL
                .into_iter()
                .map(|source| descriptor(source, probe.state.clone()))
                .collect()
        })
    }

    fn read(&mut self, source: Source, deadline_ms: u32) -> ReadFuture<'_, Reading> {
        Box::pin(async move {
            match source {
                Source::Statements => self.ledger.statements(),
                Source::Database => round_trip(self, deadline_ms).await,
                other => Reading::missing(unavailable(other)),
            }
        })
    }

    fn cancellation(&self) -> ReadFuture<'static, ()> {
        let cancel = self.cancel.clone();
        Box::pin(async move {
            let _ = cancel.cancel().await;
        })
    }
}

fn descriptor(source: Source, probe: SampleState) -> Descriptor {
    let (state, coverage) = match source {
        Source::Database => (
            probe,
            format!(
                "{OWN} SELECT 1 round trip on the monitor stream plus own request totals; server CPU, memory and storage are unavailable."
            ),
        ),
        Source::Statements => (
            SampleState::Ok,
            format!(
                "{OWN} Client wall time, returned rows and errors per statement shape; Hrana responses carry no server timings that OpenRows reads."
            ),
        ),
        other => (unavailable(other), OWN.to_owned()),
    };
    Descriptor {
        source,
        state,
        scope: Scope::Database,
        interval_ms: source.floor_ms(5_000),
        guidance: None,
        coverage,
    }
}

fn unavailable(source: Source) -> SampleState {
    let reason = match source {
        Source::Activity => "Other clients' sessions, locks and slow queries",
        Source::Io => "Server OS, CPU, memory, storage and wait metrics",
        Source::Replication => "Replica and embedded-replica sync health",
        Source::Tables | Source::Indexes => "Table and index usage counters",
        Source::Database | Source::Statements => "This source",
    };
    SampleState::Unavailable(format!("{reason}: unavailable. {OWN}"))
}

/// Time `SELECT 1`; at three quarters of the deadline the request is aborted
/// and the remaining quarter lets the stream settle.
async fn round_trip(session: &mut LibsqlSession, deadline: u32) -> Reading {
    let deadline = u64::from(deadline.clamp(4, 5_000));
    let cancel = session.cancel.clone();
    let started = Instant::now();
    let outcome = {
        let mut probe = std::pin::pin!(session.text_query("SELECT 1", &[]));
        let first = Duration::from_millis(deadline * 3 / 4);
        if let Ok(result) = tokio::time::timeout(first, &mut probe).await {
            Some(result)
        } else {
            let _ = cancel.cancel().await;
            let rest = Duration::from_millis(deadline / 4);
            tokio::time::timeout(rest, &mut probe).await.ok()
        }
    };
    let elapsed = started.elapsed().as_secs_f64() * 1_000.0;
    match outcome {
        Some(Ok(_)) => reading(elapsed, session.ledger.totals(), session.ledger.epoch()),
        Some(Err(DriverError::Auth(_))) => Reading::missing(SampleState::PermissionDenied(
            "The database token was refused; no new token is requested".into(),
        )),
        Some(Err(_)) | None => {
            let mut reading = Reading::missing(SampleState::Gap);
            reading.terminal = !session.usable();
            reading
        }
    }
}

fn reading(round_trip_ms: f64, totals: Totals, epoch: &str) -> Reading {
    let mut columns = vec![
        Column {
            name: "database_id".into(),
            unit: "text".into(),
            kind: MetricKind::Text,
        },
        Column {
            name: "round_trip_ms".into(),
            unit: "ms".into(),
            kind: MetricKind::Gauge,
        },
    ];
    columns.extend(Totals::columns());
    let mut row = vec![
        Some("remote".to_owned()),
        Some(format!("{round_trip_ms:.3}")),
    ];
    row.extend(totals.cells());
    Reading {
        terminal: false,
        state: SampleState::Ok,
        epoch: epoch.to_owned(),
        columns,
        rows: vec![row],
        partial: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_os_and_other_client_sources_are_explicitly_unavailable() {
        for source in [
            Source::Activity,
            Source::Io,
            Source::Replication,
            Source::Tables,
            Source::Indexes,
        ] {
            let descriptor = descriptor(source, SampleState::Ok);
            assert!(
                matches!(&descriptor.state, SampleState::Unavailable(reason) if reason.ends_with(OWN)),
                "{source:?}"
            );
        }
        let io = descriptor(Source::Io, SampleState::Ok);
        assert!(
            matches!(io.state, SampleState::Unavailable(reason) if reason.contains("CPU, memory"))
        );
    }

    #[test]
    fn database_reading_holds_only_measured_client_values() {
        let totals = Totals {
            requests: 3,
            errors: 1,
            total_ms: 12.5,
            rows: 7,
        };
        let reading = reading(4.25, totals, "own-1");
        let names: Vec<_> = reading.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "database_id",
                "round_trip_ms",
                "requests",
                "request_errors",
                "request_ms",
                "rows_returned"
            ]
        );
        assert!(
            !names
                .iter()
                .any(|n| n.contains("cpu") || n.contains("memory"))
        );
        assert_eq!(
            reading.rows,
            [[
                Some("remote".into()),
                Some("4.250".into()),
                Some("3".into()),
                Some("1".into()),
                Some("12.500".into()),
                Some("7".into())
            ]]
        );
        assert_eq!(reading.epoch, "own-1");
    }

    #[test]
    fn ledger_key_never_carries_credentials() -> Result<(), url::ParseError> {
        let a = ledger(&Url::parse("https://user:secret@db.example.test/x")?);
        let b = ledger(&Url::parse("https://db.example.test/x")?);
        assert!(std::sync::Arc::ptr_eq(&a, &b));
        Ok(())
    }
}
