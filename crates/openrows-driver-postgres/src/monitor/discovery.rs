// SPDX-License-Identifier: Apache-2.0
//! At most twelve bounded discovery reads, with explicit role/settings states.
use super::{Config, guarded, sources};
use crate::PgSession;
use openrows_driver_api::DriverError;
use openrows_monitor::{Descriptor, SampleState, Scope, Source};

const SETTINGS: &str = "SELECT current_setting('server_version_num'), (SELECT rolsuper FROM pg_roles WHERE rolname = current_user) OR pg_has_role(current_user, 'pg_read_all_stats', 'USAGE'), current_setting('track_activities'), current_setting('track_counts'), current_setting('track_io_timing'), current_setting('pg_stat_statements.track', true), current_setting('compute_query_id', true)";
const EXTENSION: &str = "SELECT (SELECT n.nspname FROM pg_extension e JOIN pg_namespace n ON n.oid = e.extnamespace WHERE e.extname = 'pg_stat_statements'), EXISTS(SELECT 1 FROM pg_available_extensions WHERE name = 'pg_stat_statements')";
const COLUMNS: &str = "SELECT a.attname FROM pg_attribute a JOIN pg_class c ON c.oid = a.attrelid JOIN pg_namespace n ON n.oid = c.relnamespace JOIN pg_extension e ON e.extnamespace = n.oid AND e.extname = 'pg_stat_statements' WHERE c.relname = 'pg_stat_statements' AND a.attnum > 0 AND NOT a.attisdropped ORDER BY a.attnum LIMIT 100";
const INFO: &str = "SELECT EXISTS(SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace JOIN pg_extension e ON e.extnamespace = n.oid AND e.extname = 'pg_stat_statements' WHERE c.relname = 'pg_stat_statements_info')";

pub(super) async fn discover(session: &mut PgSession) -> Vec<Descriptor> {
    let settings = guarded(session, SETTINGS, 1_000).await;
    let mut config = Config {
        descriptors: Vec::new(),
        schema: None,
        columns: Vec::new(),
        statement_epoch: false,
        io_bytes: false,
    };
    if let Ok(rows) = &settings {
        let values = rows.first().map(Vec::as_slice).unwrap_or_default();
        config.io_bytes = values
            .first()
            .and_then(Option::as_deref)
            .and_then(|v| v.parse::<u32>().ok())
            .is_some_and(|v| v >= 180_000);
        initialize(session, &mut config, values).await;
    } else {
        let state = settings
            .as_ref()
            .err()
            .map_or(SampleState::Unknown, |e| super::failure(e).state);
        config.descriptors = Source::ALL
            .into_iter()
            .map(|source| descriptor(source, state.clone()))
            .collect();
    }
    let descriptors = config.descriptors.clone();
    session.monitor_config = Some(config);
    descriptors
}

async fn initialize(session: &mut PgSession, config: &mut Config, values: &[Option<String>]) {
    let extension = statement_config(session, config).await;
    for source in Source::ALL {
        let state = if source == Source::Statements {
            match &extension {
                Ok(extensions) => initial_state(source, values, config, extensions),
                Err(error) => super::failure(error).state,
            }
        } else {
            initial_state(source, values, config, &[])
        };
        let mut descriptor = descriptor(source, state);
        probe(session, config, &mut descriptor).await;
        config.descriptors.push(descriptor);
    }
}

async fn statement_config(
    session: &mut PgSession,
    config: &mut Config,
) -> Result<Vec<Vec<Option<String>>>, DriverError> {
    let extensions = guarded(session, EXTENSION, 1_000).await?;
    config.schema = extensions
        .first()
        .and_then(|row| row.first())
        .cloned()
        .flatten();
    config.columns = guarded(session, COLUMNS, 1_000)
        .await?
        .into_iter()
        .filter_map(|row| row.into_iter().next().flatten())
        .collect();
    config.statement_epoch = guarded(session, INFO, 1_000)
        .await?
        .first()
        .and_then(|row| row.first())
        .and_then(Option::as_deref)
        == Some("t");
    Ok(extensions)
}

fn initial_state(
    source: Source,
    values: &[Option<String>],
    config: &Config,
    extensions: &[Vec<Option<String>>],
) -> SampleState {
    let value = |index| values.get(index).and_then(Option::as_deref);
    let version = value(0).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
    if !(120_000..190_000).contains(&version) {
        return unavailable("Only probed PostgreSQL 12–18 layouts are supported");
    }
    if matches!(
        source,
        Source::Activity | Source::Replication | Source::Statements
    ) && value(1) != Some("t")
    {
        return SampleState::PermissionDenied(
            "Other-user statistics require pg_read_all_stats (included by pg_monitor)".into(),
        );
    }
    match source {
        Source::Activity if value(2) != Some("on") => unavailable("track_activities is disabled"),
        Source::Database | Source::Tables | Source::Indexes if value(3) != Some("on") => {
            unavailable("track_counts is disabled")
        }
        Source::Io if version < 160_000 => unavailable(
            "pg_stat_io requires PostgreSQL 16+; database block counters remain available",
        ),
        Source::Statements => statement_state(value(5), value(6), config, extensions),
        Source::Activity
        | Source::Database
        | Source::Tables
        | Source::Indexes
        | Source::Replication
        | Source::Io => SampleState::Ok,
    }
}

fn statement_state(
    track: Option<&str>,
    query_id: Option<&str>,
    config: &Config,
    extensions: &[Vec<Option<String>>],
) -> SampleState {
    if config.schema.is_none() {
        let available = extensions
            .first()
            .and_then(|row| row.get(1))
            .and_then(Option::as_deref)
            == Some("t");
        return unavailable(if available {
            "pg_stat_statements is not created in this database"
        } else {
            "pg_stat_statements package is unavailable"
        });
    }
    if track.is_none() {
        return unavailable(
            "pg_stat_statements is installed but not preloaded; DBA configuration and restart required",
        );
    }
    if track == Some("none") {
        return unavailable("pg_stat_statements tracking is disabled");
    }
    if query_id == Some("off") {
        return unavailable(
            "compute_query_id is off; a compatible query-ID provider must be reviewed",
        );
    }
    if !config
        .columns
        .iter()
        .any(|c| c == "total_time" || c == "total_exec_time")
    {
        return unavailable("Statement statistics columns are unsupported");
    }
    SampleState::Ok
}

fn unavailable(reason: &str) -> SampleState {
    SampleState::Unavailable(reason.into())
}

fn descriptor(source: Source, state: SampleState) -> Descriptor {
    let guidance = match source {
        Source::Activity | Source::Replication => Some("Ask the DBA to review: GRANT pg_read_all_stats TO \"monitor_role\"; pg_monitor includes this role. Never use superuser for routine collection.".into()),
        Source::Statements => Some("Ask the DBA/provider to configure shared_preload_libraries with pg_stat_statements and restart, then review in the selected database: CREATE EXTENSION pg_stat_statements; GRANT pg_read_all_stats TO \"monitor_role\"; tracking/query IDs must be enabled. Changes have overhead; OpenRows never applies them.".into()),
        Source::Database | Source::Tables | Source::Indexes | Source::Io => Some("Ask the DBA to review track_counts and statistics SELECT access. No collector changes settings or grants.".into()),
    };
    Descriptor { source, state, scope: if matches!(source, Source::Activity | Source::Replication | Source::Io) { Scope::Server } else { Scope::Database }, interval_ms: source.floor_ms(5_000), guidance,
        coverage: "Sampled statistics; capped rows are partial. No per-query CPU/memory or latency percentiles. Block counters do not measure physical disk I/O. Statement continuity without a reset epoch is best effort; source maxima are cumulative. I/O timing is not supplied when disabled. Activity excludes collector sessions; statement counters can include collector and guard commands. Rankings cover the latest valid interval, not the full live window.".into() }
}

async fn probe(session: &mut PgSession, config: &Config, descriptor: &mut Descriptor) {
    if descriptor.state != SampleState::Ok {
        return;
    }
    let Some((sql, _)) = sources::query(descriptor.source, config) else {
        return;
    };
    if let Err(error) = guarded(session, &sql, 1_000).await {
        descriptor.state = super::failure(&error).state;
    }
}
