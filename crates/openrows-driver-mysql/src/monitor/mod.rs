// SPDX-License-Identifier: Apache-2.0
//! Read-only `MySQL`/`MariaDB` collectors, separately probed; core owns query redaction.
mod activity;
mod bounded;
mod discovery;
mod guard;
mod layout;
mod replication;
mod sources;
mod status;
#[cfg(test)]
mod tests;
use crate::MySqlSession;
use layout::Layout;
use openrows_driver_api::{DriverError, Session};
use openrows_monitor::{Descriptor, MonitorReader, ReadFuture, Reading, SampleState, Source};
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SqlText {
    Safe,
    FingerprintOnly,
}

/// Probed engine layout and source availability for one dedicated session.
pub(crate) struct Config {
    layout: Layout,
    performance: bool,
    probed: Probed,
    text: SqlText,
    descriptors: Vec<Descriptor>,
    epochs: BTreeMap<Source, status::Epoch>,
}
/// Optional sources found by discovery probes; absent ones stay explicit gaps.
#[derive(Default)]
struct Probed {
    waits: bool,
    sys_indexes: bool,
    deadlocks: bool,
}
impl MonitorReader for MySqlSession {
    fn discover(&mut self) -> ReadFuture<'_, Vec<Descriptor>> {
        Box::pin(discovery::discover(self))
    }
    fn read(&mut self, source: Source, deadline_ms: u32) -> ReadFuture<'_, Reading> {
        Box::pin(sample(self, source, deadline_ms))
    }
    fn cancellation(&self) -> ReadFuture<'static, ()> {
        let cancel = self.cancel_handle();
        Box::pin(async move {
            if let Some(cancel) = cancel {
                let _ = cancel.cancel().await;
            }
        })
    }
}
async fn sample(session: &mut MySqlSession, source: Source, deadline: u32) -> Reading {
    let Some(config) = &session.monitor_config else {
        return Reading::missing(layout::unavailable(
            "Explicitly start monitoring to probe sources",
        ));
    };
    let state = config
        .descriptors
        .iter()
        .find(|d| d.source == source)
        .map_or(SampleState::Unknown, |d| d.state.clone());
    if state != SampleState::Ok {
        return Reading::missing(state);
    }
    let (sql, columns) = sources::query(source, config);
    let layout = config.layout;
    let result = match guard::guarded(session, layout, &sql, deadline).await {
        Ok(result) => result,
        Err(error) => return failure(&error, source, layout),
    };
    let Some(config) = &mut session.monitor_config else {
        return Reading::missing(SampleState::Gap);
    };
    assemble(source, result, config, columns)
}
fn assemble(
    source: Source,
    result: bounded::Bounded,
    config: &mut Config,
    columns: Vec<openrows_monitor::Column>,
) -> Reading {
    let safe_text = config.text == SqlText::Safe;
    let epoch = config.epochs.entry(source).or_default();
    match source {
        Source::Database => return status::reading(result, epoch),
        Source::Replication => return replication::reading(&result),
        _ => {}
    }
    let mut rows = result.rows;
    // Every projection ends with the instrumentation flag, then uptime.
    let uptime = trailing(&rows, 1).and_then(|v| v.parse().ok());
    if trailing(&rows, 2).is_some_and(|v| v != "1") {
        return Reading::missing(layout::unavailable(
            "Required performance_schema instrumentation was disabled; explicitly re-probe",
        ));
    }
    for row in &mut rows {
        row.truncate(row.len().saturating_sub(2));
        if !safe_text {
            suppress_queries(row, &columns);
        }
    }
    Reading {
        terminal: false,
        state: SampleState::Ok,
        epoch: epoch.observe(uptime, None),
        partial: result.partial || capped(source, &rows),
        rows,
        columns,
    }
}
/// The `back`-th value from the end of the first row.
fn trailing(rows: &[Vec<Option<String>>], back: usize) -> Option<&str> {
    let row = rows.first()?;
    row.get(row.len().checked_sub(back)?)?.as_deref()
}
/// Row caps, or the 100-edge `InnoDB` blocking cap, make coverage partial.
fn capped(source: Source, rows: &[Vec<Option<String>>]) -> bool {
    let limit = if source == Source::Statements {
        200
    } else {
        1_000
    };
    let edges = rows
        .iter()
        .filter_map(|r| r.get(10).and_then(Option::as_deref)?.parse::<u64>().ok())
        .sum::<u64>();
    rows.len() >= limit || (source == Source::Activity && edges >= 100)
}
fn suppress_queries(row: &mut [Option<String>], columns: &[openrows_monitor::Column]) {
    for (cell, _) in row
        .iter_mut()
        .zip(columns)
        .filter(|(_, column)| column.kind == openrows_monitor::MetricKind::Query)
    {
        *cell = None;
    }
}

/// Table, column, global-privilege and routine denials (database denials are `Auth`).
const DENIED: [u16; 4] = [1142, 1143, 1227, 1370];
/// Query interrupted, `MariaDB` `max_statement_time`, `MySQL` `max_execution_time`.
const DEADLINE: [u16; 3] = [1317, 1969, 3024];

fn failure(error: &DriverError, source: Source, layout: Layout) -> Reading {
    let code = server_code(error);
    let state =
        if matches!(error, DriverError::Auth(_)) || code.is_some_and(|c| DENIED.contains(&c)) {
            SampleState::PermissionDenied(format!(
                "Ask the DBA to review {}; no grants are changed",
                grant(source, layout)
            ))
        } else if code.is_none_or(|c| DEADLINE.contains(&c)) {
            SampleState::Gap
        } else {
            layout::unavailable(
                "Source layout or deadline guard unavailable; explicitly re-probe settings/access",
            )
        };
    let mut reading = Reading::missing(state);
    reading.terminal = !matches!(error, DriverError::Server { .. } | DriverError::Auth(_));
    reading.partial =
        matches!(error, DriverError::Protocol(reason) if reason.contains("limit exceeded"));
    reading
}
/// The numeric server error code that `error::classify` appends to the message.
fn server_code(error: &DriverError) -> Option<u16> {
    let DriverError::Server { message, .. } = error else {
        return None;
    };
    let (_, code) = message.rsplit_once("(MySQL error ")?;
    code.strip_suffix(')')?.parse().ok()
}
fn grant(source: Source, layout: Layout) -> &'static str {
    match source {
        Source::Activity => "PROCESS and SELECT on the required performance_schema tables",
        Source::Replication => layout.replication_grant(),
        Source::Database => "server status visibility",
        Source::Tables | Source::Indexes | Source::Statements | Source::Io => {
            "SELECT on the required performance_schema tables (and sys views if used)"
        }
    }
}
