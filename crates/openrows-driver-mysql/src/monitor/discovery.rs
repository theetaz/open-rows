// SPDX-License-Identifier: Apache-2.0
//! At most fourteen bounded discovery reads; setup guidance never executes mutations.
use super::{
    Config, Probed, SqlText, guard,
    layout::{Layout, unavailable},
    sources, status,
};
use crate::MySqlSession;
use openrows_monitor::{Descriptor, SampleState, Scope, Source};
use std::collections::BTreeMap;

const SETTINGS: &str = "SELECT VERSION(), @@performance_schema, @@sql_mode";
const PROCESS: &str = "SELECT COUNT(*) FROM information_schema.INNODB_TRX";
const WAITS: &str = "SELECT THREAD_ID, EVENT_NAME, END_EVENT_ID FROM performance_schema.events_waits_current LIMIT 0";
const GUIDANCE: &str = "Review with DBA: PROCESS for all-session activity/locks; SELECT on required performance_schema/sys sources; MySQL REPLICATION CLIENT or MariaDB SLAVE MONITOR for replication. performance_schema=ON is a server-start setting and needs restart. Review instruments/consumers separately; OpenRows never enables or grants them.";
const COVERAGE: &str = "Bounded best-effort observations; uptime decreases/counter decreases reset baselines; truncation/eviction can lose continuity. No per-query CPU/memory or latency percentiles; no automatic setup.";
const INSTRUMENTS: &str = "SELECT SUM(NAME LIKE 'statement/%' AND ENABLED = 'YES' AND TIMED = 'YES'), SUM(NAME LIKE 'wait/io/table/%' AND ENABLED = 'YES' AND TIMED = 'YES'), SUM(NAME LIKE 'wait/io/file/%' AND ENABLED = 'YES' AND TIMED = 'YES'), (SELECT COUNT(*) FROM performance_schema.setup_consumers WHERE NAME IN ('global_instrumentation', 'thread_instrumentation', 'statements_digest') AND ENABLED = 'YES'), (SELECT COUNT(*) FROM performance_schema.setup_consumers WHERE NAME = 'events_waits_current' AND ENABLED = 'YES'), (SELECT COUNT(*) FROM performance_schema.setup_consumers WHERE NAME IN ('global_instrumentation', 'thread_instrumentation') AND ENABLED = 'YES') FROM performance_schema.setup_instruments";

pub(super) async fn discover(session: &mut MySqlSession) -> Vec<Descriptor> {
    session.monitor_config = None;
    let settings = match guard::query(session, SETTINGS, None).await {
        Ok(result) => result.rows.into_iter().next().unwrap_or_default(),
        Err(_) => return missing("Version/settings probe failed; reconnect to re-probe"),
    };
    let Some(layout) = settings
        .first()
        .and_then(Option::as_deref)
        .and_then(Layout::detect)
    else {
        return missing("Supported layouts: MySQL 5.7/8.0/8.4/9.x and MariaDB 10.6–11.x");
    };
    let mut config = Config {
        layout,
        performance: settings.get(1).and_then(Option::as_deref) == Some("1"),
        text: if settings
            .get(2)
            .and_then(Option::as_deref)
            .is_some_and(|m| !m.contains("NO_BACKSLASH_ESCAPES") && !m.contains("ANSI_QUOTES"))
        {
            SqlText::Safe
        } else {
            SqlText::FingerprintOnly
        },
        probed: Probed::default(),
        descriptors: Vec::new(),
        epochs: BTreeMap::new(),
    };
    let instrumentation = instrumentation(session, &mut config).await;
    probe_optional(session, &mut config).await;
    for source in Source::ALL {
        let mut descriptor = descriptor(source, initial(source, &config, &instrumentation));
        if descriptor.state == SampleState::Ok {
            descriptor.state = probe(session, source, &config).await;
        }
        coverage(&mut descriptor, &config);
        config.descriptors.push(descriptor);
    }
    let descriptors = config.descriptors.clone();
    session.monitor_config = Some(config);
    descriptors
}
/// Probe one source; activity also needs PROCESS, because without it both
/// engines silently list only the caller's own threads instead of failing.
async fn probe(session: &mut MySqlSession, source: Source, config: &Config) -> SampleState {
    let layout = config.layout;
    if source == Source::Activity
        && let Err(error) = guard::guarded(session, layout, PROCESS, 1_000).await
    {
        return super::failure(&error, source, layout).state;
    }
    let (sql, _) = sources::query(source, config);
    match guard::guarded(session, layout, &sql, 1_000).await {
        Ok(_) => SampleState::Ok,
        Err(error) => super::failure(&error, source, layout).state,
    }
}
async fn instrumentation(
    session: &mut MySqlSession,
    config: &mut Config,
) -> Result<Vec<Option<String>>, SampleState> {
    if !config.performance {
        return Ok(Vec::new());
    }
    let values = guard::guarded(session, config.layout, INSTRUMENTS, 1_000)
        .await
        .map_err(|e| super::failure(&e, Source::Statements, config.layout).state)?
        .rows
        .into_iter()
        .next()
        .unwrap_or_default();
    let value = |index: usize| values.get(index).and_then(Option::as_deref);
    config.probed.waits = value(5) == Some("2")
        && value(4) == Some("1")
        && guard::guarded(session, config.layout, WAITS, 1_000)
            .await
            .is_ok();
    Ok(values)
}
/// Optional `sys` index health and, on `MySQL`, the `InnoDB` deadlock metric.
async fn probe_optional(session: &mut MySqlSession, config: &mut Config) {
    if !config.performance {
        return;
    }
    let unused =
        "SELECT object_schema, object_name, index_name FROM sys.schema_unused_indexes LIMIT 0";
    let redundant = "SELECT table_schema, table_name, redundant_index_name, dominant_index_name FROM sys.schema_redundant_indexes LIMIT 0";
    config.probed.sys_indexes = guard::guarded(session, config.layout, unused, 1_000)
        .await
        .is_ok()
        && guard::guarded(session, config.layout, redundant, 1_000)
            .await
            .is_ok();
    if !config.layout.maria {
        config.probed.deadlocks = guard::guarded(session, config.layout, &status::mysql(), 1_000)
            .await
            .is_ok_and(|result| status::has_deadlocks(&result));
    }
}
pub(super) fn initial(
    source: Source,
    config: &Config,
    instruments: &Result<Vec<Option<String>>, SampleState>,
) -> SampleState {
    if matches!(
        source,
        Source::Activity | Source::Database | Source::Replication
    ) {
        return SampleState::Ok;
    }
    if !config.performance {
        return unavailable(
            "performance_schema disabled; enabling requires DBA configuration and server restart",
        );
    }
    match instruments {
        Err(state) => state.clone(),
        Ok(values) if enabled(source, values) => SampleState::Ok,
        Ok(_) => unavailable(
            "Required performance_schema instruments/timing/consumers disabled; ask DBA to review setup",
        ),
    }
}
/// Whether the `INSTRUMENTS` row (timed statement/table/file instruments, statement
/// consumers, waits-current consumer, thread consumers) feeds `source`.
pub(super) fn enabled(source: Source, values: &[Option<String>]) -> bool {
    let value = |index: usize| values.get(index).and_then(Option::as_deref);
    let timed = |index: usize| {
        value(index)
            .and_then(|v| v.parse::<u64>().ok())
            .is_some_and(|v| v > 0)
    };
    let threads = value(5) == Some("2");
    match source {
        Source::Statements => timed(0) && value(3) == Some("3"),
        Source::Tables | Source::Indexes => timed(1) && threads,
        Source::Io => timed(2) && threads,
        Source::Activity | Source::Database | Source::Replication => true,
    }
}
fn descriptor(source: Source, state: SampleState) -> Descriptor {
    let scope = if matches!(
        source,
        Source::Tables | Source::Indexes | Source::Statements
    ) {
        Scope::Database
    } else {
        Scope::Server
    };
    Descriptor {
        source,
        state,
        scope,
        interval_ms: source.floor_ms(5_000),
        guidance: Some(GUIDANCE.into()),
        coverage: COVERAGE.into(),
    }
}
fn coverage(descriptor: &mut Descriptor, config: &Config) {
    if descriptor.source == Source::Activity {
        if !config.probed.waits {
            descriptor
                .coverage
                .push_str(" Instrumented wait events unavailable.");
        }
        if config.layout.modern_locks && !config.performance {
            descriptor
                .coverage
                .push_str(" InnoDB blocking unavailable with performance_schema disabled.");
        }
        descriptor
            .coverage
            .push_str(" At most 100 InnoDB blocking edges; metadata locks are not included.");
    }
    if descriptor.source == Source::Database && !config.layout.maria && !config.probed.deadlocks {
        descriptor.coverage.push_str(" MySQL deadlocks come only from the enabled INNODB_METRICS lock_deadlocks counter (PROCESS, performance_schema on); unknown otherwise.");
    }
    if descriptor.source == Source::Indexes {
        if !config.probed.sys_indexes {
            descriptor.coverage.push_str(" sys unused/redundant index health unavailable; table/index I/O remains available.");
        }
        descriptor
            .coverage
            .push_str(" Zero reads since reset is not proof an index is unused; never auto-drop.");
    }
    if descriptor.source == Source::Statements {
        descriptor.coverage.push_str(" Top-N covers 200 digests in the selected schema, ordered by cumulative total; statistics include monitor queries.");
    }
    if config.text == SqlText::FingerprintOnly {
        descriptor
            .coverage
            .push_str(" SQL mode prevents safe normalisation; fingerprint/metrics only.");
    }
}
fn missing(reason: &str) -> Vec<Descriptor> {
    Source::ALL
        .into_iter()
        .map(|s| descriptor(s, unavailable(reason)))
        .collect()
}
