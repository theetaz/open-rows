// SPDX-License-Identifier: Apache-2.0
//! Read-only `ClickHouse` collectors on the connected node; core owns query redaction.
mod counters;
mod discovery;
mod guard;
mod sources;
#[cfg(test)]
mod tests;
use crate::session::ClickHouseSession;
use openrows_driver_api::{DriverError, Session};
use openrows_monitor::{
    Column, Descriptor, MetricKind, MonitorReader, ReadFuture, Reading, SampleState, Source,
};

/// Probed scope and source availability for one dedicated session.
pub(crate) struct Config {
    /// Cluster read through `clusterAllReplicas`; `None` is the connected node.
    cluster: Option<String>,
    /// Readable counter tables: events, metrics, asynchronous metrics.
    counters: [bool; 3],
    descriptors: Vec<Descriptor>,
    epoch: counters::Epoch,
}
impl MonitorReader for ClickHouseSession {
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
async fn sample(session: &mut ClickHouseSession, source: Source, deadline: u32) -> Reading {
    let Some(config) = &session.monitor_config else {
        return Reading::missing(unavailable("Explicitly start monitoring to probe sources"));
    };
    let state = config
        .descriptors
        .iter()
        .find(|d| d.source == source)
        .map_or(SampleState::Unknown, |d| d.state.clone());
    if state != SampleState::Ok {
        return Reading::missing(state);
    }
    let (sql, spec) = sources::query(source, config);
    let result = match guard::query(session, &sql, deadline).await {
        Ok(result) => result,
        Err(error) => return failure(&error, source),
    };
    let Some(config) = &mut session.monitor_config else {
        return Reading::missing(SampleState::Gap);
    };
    match source {
        Source::Database => counters::reading(result, &mut config.epoch),
        Source::Replication => sources::replication(result, columns(spec)),
        _ => sources::reading(source, result, columns(spec)),
    }
}

/// Error numbers that mean a deadline, read/result cap or memory guard stopped the read.
const BOUNDED: [u32; 6] = [158, 159, 202, 241, 394, 396];

fn failure(error: &DriverError, source: Source) -> Reading {
    let code = server_code(error);
    let state = if matches!(error, DriverError::Auth(_)) || code == Some(497) {
        SampleState::PermissionDenied(format!(
            "Ask the DBA to review {}; no grants are changed",
            grant(source)
        ))
    } else if code == Some(60) {
        unavailable(missing_table(source))
    } else if code.is_none_or(|c| BOUNDED.contains(&c)) {
        SampleState::Gap
    } else {
        unavailable(
            "Source layout or guard settings unavailable (readonly profile or settings constraints); explicitly re-probe",
        )
    };
    let mut reading = Reading::missing(state);
    reading.partial = code.is_some_and(|c| matches!(c, 158 | 396))
        || matches!(error, DriverError::Protocol(reason) if reason.contains("size limit"));
    reading
}
/// The `ClickHouse` error number that `error::server` appends to the message.
fn server_code(error: &DriverError) -> Option<u32> {
    let DriverError::Server { message, .. } = error else {
        return None;
    };
    let (_, code) = message.rsplit_once("(ClickHouse error ")?;
    code.strip_suffix(')')?.parse().ok()
}
fn grant(source: Source) -> &'static str {
    match source {
        Source::Activity => "SELECT on system.processes (and REMOTE for cluster scope)",
        Source::Database => {
            "SELECT on system.events, system.metrics and system.asynchronous_metrics"
        }
        Source::Statements => "SELECT on system.query_log",
        Source::Tables => "SELECT on system.parts",
        Source::Io => "SELECT on system.merges",
        Source::Replication => "SELECT on system.replicas",
        Source::Indexes => "nothing; ClickHouse has no index usage statistics",
    }
}
fn missing_table(source: Source) -> &'static str {
    if source == Source::Statements {
        "system.query_log is not configured on this node, or has not flushed yet; enabling it is DBA server configuration. Explicitly re-probe later"
    } else {
        "System table missing on this server version or configuration"
    }
}
pub(super) fn unavailable(reason: &str) -> SampleState {
    SampleState::Unavailable(reason.into())
}
/// `name:unit` pairs; `counter` becomes a cumulative count/bytes/ms column.
pub(super) fn columns(spec: &str) -> Vec<Column> {
    spec.split(',')
        .filter_map(|part| part.split_once(':'))
        .map(|(name, unit)| Column {
            name: name.into(),
            unit: match unit {
                "counter" if name.ends_with("ms") => "ms",
                "counter" if name.ends_with("bytes") => "bytes",
                "counter" => "count",
                other => other,
            }
            .into(),
            kind: match unit {
                "counter" => MetricKind::Counter,
                "text" => MetricKind::Text,
                "query" => MetricKind::Query,
                _ => MetricKind::Gauge,
            },
        })
        .collect()
}
