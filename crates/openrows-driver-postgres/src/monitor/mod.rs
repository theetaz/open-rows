// SPDX-License-Identifier: Apache-2.0
//! `PostgreSQL` 12–18 guarded source adapter; no grants/setup/reset side effects.
mod discovery;
mod sources;
use crate::{PgSession, simple};
use openrows_driver_api::{DriverError, Session};
use openrows_monitor::{Descriptor, MonitorReader, ReadFuture, Reading, SampleState, Source};

pub(crate) struct Config {
    pub(crate) descriptors: Vec<Descriptor>,
    pub(crate) schema: Option<String>,
    pub(crate) columns: Vec<String>,
    pub(crate) statement_epoch: bool,
    pub(crate) io_bytes: bool,
}

impl MonitorReader for PgSession {
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

pub(crate) async fn guarded(
    session: &mut PgSession,
    sql: &str,
    deadline_ms: u32,
) -> Result<Vec<simple::TextRow>, DriverError> {
    session.wire.limit_messages(1_024 * 1_024);
    let guard = format!(
        "BEGIN READ ONLY; SET LOCAL search_path = pg_catalog; SET LOCAL statement_timeout = {}; SET LOCAL lock_timeout = 100",
        deadline_ms.clamp(1, 1_000)
    );
    if let Err(error) = simple::query_limited(&mut session.wire, &guard, 1_024 * 1_024).await {
        return finish(session, Err(error), "ROLLBACK").await;
    }
    let result = simple::query_limited(&mut session.wire, sql, 1_024 * 1_024).await;
    let command = if result.is_ok() { "COMMIT" } else { "ROLLBACK" };
    finish(session, result, command).await
}

async fn finish(
    session: &mut PgSession,
    result: Result<Vec<simple::TextRow>, DriverError>,
    command: &str,
) -> Result<Vec<simple::TextRow>, DriverError> {
    if result
        .as_ref()
        .err()
        .is_some_and(|error| !matches!(error, DriverError::Server { .. }))
    {
        session.wire.invalidate();
        return result;
    }
    if let Err(error) = simple::query_limited(&mut session.wire, command, 1_024 * 1_024).await {
        session.wire.invalidate();
        return Err(error);
    }
    result
}

async fn sample(session: &mut PgSession, source: Source, deadline: u32) -> Reading {
    let Some(config) = &session.monitor_config else {
        return Reading::missing(SampleState::Unavailable(
            "Start monitoring to probe sources".into(),
        ));
    };
    let Some(descriptor) = config.descriptors.iter().find(|d| d.source == source) else {
        return Reading::missing(SampleState::Unknown);
    };
    if descriptor.state != SampleState::Ok {
        return Reading::missing(descriptor.state.clone());
    }
    let Some((sql, columns)) = sources::query(source, config) else {
        return Reading::missing(SampleState::Unavailable(
            "Source layout is unsupported".into(),
        ));
    };
    let sql = visibility_query(source, &sql, &columns);
    match guarded(session, &sql, deadline).await {
        Ok(mut rows) => {
            let visibility = rows
                .first()
                .and_then(|row| row.get(columns.len()))
                .and_then(Option::as_deref);
            if visibility == Some("f") {
                return Reading::missing(SampleState::PermissionDenied(
                    "Statistics visibility was revoked; ask the DBA for pg_read_all_stats".into(),
                ));
            }
            if rows
                .first()
                .and_then(|row| row.get(columns.len() + 1))
                .and_then(Option::as_deref)
                == Some("f")
            {
                return Reading::missing(SampleState::Unavailable(
                    "Statistics tracking was disabled; explicitly re-probe settings".into(),
                ));
            }
            let epoch = rows
                .first()
                .and_then(|row| row.last())
                .and_then(Option::as_deref)
                .unwrap_or("best effort")
                .to_owned();
            for row in &mut rows {
                row.pop();
                row.pop();
                row.pop();
            }
            rows.retain(|row| row.first().is_some_and(Option::is_some));
            let state = if rows.is_empty() && source == Source::Replication {
                SampleState::Unavailable("Not applicable: no replication sender or receiver".into())
            } else {
                SampleState::Ok
            };
            Reading {
                terminal: false,
                state,
                epoch,
                partial: rows.len() >= 1_000,
                rows,
                columns,
            }
        }
        Err(error) => failure(&error),
    }
}

fn visibility_query(source: Source, sql: &str, columns: &[openrows_monitor::Column]) -> String {
    let names: Vec<_> = columns
        .iter()
        .map(|column| format!("\"{}\"", column.name))
        .collect();
    let names = names.join(", ");
    let visibility = if matches!(
        source,
        Source::Activity | Source::Replication | Source::Statements
    ) {
        "((SELECT rolsuper FROM pg_roles WHERE rolname = current_user) OR pg_has_role(current_user, 'pg_read_all_stats', 'USAGE'))"
    } else {
        "true"
    };
    let tracking = match source {
        Source::Activity => "current_setting('track_activities') = 'on'",
        Source::Statements => "current_setting('pg_stat_statements.track', true) <> 'none'",
        Source::Database | Source::Tables | Source::Indexes => {
            "current_setting('track_counts') = 'on'"
        }
        Source::Replication | Source::Io => "true",
    };
    format!(
        "SELECT {names}, {visibility}, {tracking}, epoch FROM (VALUES (1)) seed(n) LEFT JOIN ({sql}) data({names}, epoch) ON true LIMIT 1000"
    )
}

fn failure(error: &DriverError) -> Reading {
    let state = match error {
        DriverError::Server { code, .. } if code == "42501" => SampleState::PermissionDenied(
            "Statistics read denied; ask the DBA for the reviewed minimum grants".into(),
        ),
        DriverError::Server { code, .. } if code == "57014" || code == "55P03" => SampleState::Gap,
        DriverError::Server { .. } => SampleState::Unavailable(
            "Statistics source is unavailable; explicitly re-probe settings and access".into(),
        ),
        _ => SampleState::Gap,
    };
    let mut reading = Reading::missing(state);
    reading.partial = matches!(error, DriverError::Protocol(reason) if reason == "monitor result limit exceeded" || reason == "monitor frame limit exceeded");
    reading.terminal = !matches!(error, DriverError::Server { .. });
    reading
}
