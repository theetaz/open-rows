// SPDX-License-Identifier: Apache-2.0
//! One permission probe plus one bounded probe per source; guidance never
//! executes grants or setting changes.
use openrows_monitor::{Descriptor, SampleState, Scope, Source};

use super::platform::Platform;
use super::{Config, REPLICATION, failure, guard, sources, unavailable};
use crate::session::SqlServerSession;

const COVERAGE: &str = "Bounded best-effort DMV observations. Server start time marks the counter epoch; decreasing counters (plan-cache eviction, DBCC SQLPERF clears, database restarts) reset baselines. No automatic setup or grants.";

pub(super) async fn discover(session: &mut SqlServerSession) -> Vec<Descriptor> {
    session.monitor.config = None;
    let detected = Platform::detect(&session.summary.version, session.summary.flavour.as_deref());
    let Some(platform) = detected else {
        return missing(
            "Supported: SQL Server 2017+, Azure SQL Edge, Managed Instance and Azure SQL Database",
        );
    };
    let visible = match guard::query(session, &platform.permission_probe(), 1_000).await {
        Ok(result) => result
            .rows
            .first()
            .is_some_and(|row| row.iter().any(|cell| cell.as_deref() == Some("1"))),
        Err(_) => return missing("Permission probe failed; reconnect to re-probe"),
    };
    let mut descriptors = Vec::with_capacity(Source::ALL.len());
    for source in Source::ALL {
        let state = initial(source, platform, visible);
        let state = if state == SampleState::Ok {
            probe(session, source, platform).await
        } else {
            state
        };
        descriptors.push(descriptor(source, state, platform));
    }
    session.monitor.config = Some(Config {
        platform,
        descriptors: descriptors.clone(),
    });
    descriptors
}

/// Without the state permission SQL Server silently lists only the caller's
/// session and refuses server DMVs, so every DMV source is denied up front.
pub(super) fn initial(source: Source, platform: Platform, visible: bool) -> SampleState {
    if source == Source::Replication {
        unavailable(REPLICATION)
    } else if visible {
        SampleState::Ok
    } else {
        SampleState::PermissionDenied(format!(
            "Requires {}; ask the DBA to review it, no grants are changed",
            platform.grant()
        ))
    }
}

async fn probe(session: &mut SqlServerSession, source: Source, platform: Platform) -> SampleState {
    let Some((sql, _)) = sources::query(source, platform) else {
        return unavailable(REPLICATION);
    };
    match guard::query(session, &sql, 1_000).await {
        Ok(_) => SampleState::Ok,
        Err(error) => failure(&error, platform).state,
    }
}

fn descriptor(source: Source, state: SampleState, platform: Platform) -> Descriptor {
    let database = matches!(
        source,
        Source::Tables | Source::Indexes | Source::Statements
    ) || platform.database_scoped();
    Descriptor {
        source,
        state,
        scope: if database {
            Scope::Database
        } else {
            Scope::Server
        },
        interval_ms: source.floor_ms(5_000),
        guidance: Some(format!(
            "Review with DBA: {} for all-session activity, waits, counters and query statistics. OpenRows never grants permissions or changes server settings.",
            platform.grant()
        )),
        coverage: format!("{COVERAGE}{}", coverage(source, platform)),
    }
}

fn coverage(source: Source, platform: Platform) -> &'static str {
    match source {
        Source::Activity => {
            " User sessions only; one blocker per request from blocking_session_id (negative owner ids are not resolved). Lock-free latch and parallelism waits are listed but not chained."
        }
        Source::Database if platform.database_scoped() => {
            " Performance counters visible to this database; continuity is best effort without an instance start time."
        }
        Source::Database => {
            " Instance-wide performance counters, including collector queries. CPU is the default resource pool share; missing counters stay unknown."
        }
        Source::Statements => {
            " Top 200 query-hash groups compiled in this database by cumulative elapsed time; plan-cache eviction drops history and statistics include monitor queries."
        }
        Source::Tables | Source::Indexes => {
            " Usage since instance start or database open; never-used indexes are absent, which is not proof they are unused. Never auto-drop."
        }
        Source::Io => {
            " Top 200 non-idle waits by cumulative wait time; benign background waits are excluded."
        }
        Source::Replication => "",
    }
}

fn missing(reason: &str) -> Vec<Descriptor> {
    let platform = Platform {
        major: 0,
        edition: super::platform::Edition::OnPremises,
    };
    Source::ALL
        .into_iter()
        .map(|source| descriptor(source, unavailable(reason), platform))
        .collect()
}
