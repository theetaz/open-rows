// SPDX-License-Identifier: Apache-2.0
//! Read-only SQL Server collectors over DMVs on a dedicated session; core
//! owns query redaction. Restricted DMV visibility is an explicit state.
mod counters;
mod discovery;
mod guard;
mod platform;
mod sources;
#[cfg(test)]
mod tests;

use openrows_driver_api::{DriverError, Session};
use openrows_monitor::{
    Column, Descriptor, MonitorReader, ReadFuture, Reading, SampleState, Source,
};

use crate::session::SqlServerSession;
use platform::Platform;

/// Monitor state of one session; a lost wire is never reused.
#[derive(Default)]
pub(crate) struct State {
    config: Option<Config>,
    lost: bool,
}

/// Probed platform and source availability.
struct Config {
    platform: Platform,
    descriptors: Vec<Descriptor>,
}

const REPLICATION: &str =
    "Always On availability group and replication health are not collected for SQL Server yet";

impl MonitorReader for SqlServerSession {
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

async fn sample(session: &mut SqlServerSession, source: Source, deadline: u32) -> Reading {
    let Some(config) = &session.monitor.config else {
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
    let platform = config.platform;
    let Some((sql, columns)) = sources::query(source, platform) else {
        return Reading::missing(unavailable(REPLICATION));
    };
    match guard::query(session, &sql, deadline).await {
        Ok(result) => assemble(source, result, columns),
        Err(error) => failure(&error, platform),
    }
}

/// Strip the trailing epoch cell; an empty result has no epoch to compare.
fn assemble(source: Source, result: guard::Bounded, columns: Vec<Column>) -> Reading {
    let mut rows = result.rows;
    let epoch = rows
        .first()
        .and_then(|row| row.last().cloned().flatten())
        .unwrap_or_else(|| "best effort".into());
    for row in &mut rows {
        row.pop();
    }
    let cap = if matches!(source, Source::Statements | Source::Io) {
        200
    } else {
        1_000
    };
    Reading {
        terminal: false,
        state: SampleState::Ok,
        epoch,
        partial: result.partial || rows.len() >= cap,
        rows,
        columns,
    }
}

/// Object/server-state/database permission denials.
const DENIED: [u32; 5] = [229, 262, 297, 300, 916];
/// Unknown object, column or built-in function on this version/platform.
const MISSING: [u32; 4] = [195, 207, 208, 4121];
/// Lock request timeout.
const LOCK_TIMEOUT: u32 = 1222;

fn failure(error: &DriverError, platform: Platform) -> Reading {
    let code = server_number(error);
    let cancelled = matches!(error, DriverError::Server { code, .. } if code == "HY008");
    let state =
        if matches!(error, DriverError::Auth(_)) || code.is_some_and(|c| DENIED.contains(&c)) {
            SampleState::PermissionDenied(format!(
                "Ask the DBA to review {}; no grants are changed",
                platform.grant()
            ))
        } else if cancelled || code.is_none() || code == Some(LOCK_TIMEOUT) {
            SampleState::Gap
        } else if code.is_some_and(|c| MISSING.contains(&c)) {
            unavailable("This DMV or column is not exposed on this version/platform")
        } else {
            unavailable("Source query failed on this server; explicitly re-probe settings/access")
        };
    let mut reading = Reading::missing(state);
    reading.terminal = !matches!(error, DriverError::Server { .. } | DriverError::Auth(_));
    reading.partial =
        matches!(error, DriverError::Protocol(reason) if reason.contains("limit exceeded"));
    reading
}

/// The error number `error::describe` writes into the message.
fn server_number(error: &DriverError) -> Option<u32> {
    let DriverError::Server { message, .. } = error else {
        return None;
    };
    let (_, rest) = message.rsplit_once("(SQL Server error ")?;
    rest.split([',', ')']).next()?.parse().ok()
}

fn unavailable(reason: &str) -> SampleState {
    SampleState::Unavailable(reason.into())
}
