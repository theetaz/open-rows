// SPDX-License-Identifier: Apache-2.0
//! At most twelve bounded discovery reads; no flush, setting or grant is ever changed.
use super::{Config, counters, failure, guard, sources, unavailable};
use crate::session::ClickHouseSession;
use openrows_monitor::{Descriptor, SampleState, Scope, Source};

const SETTINGS: &str = "SELECT version(), hostName(), getSetting('log_queries_probability')";
const GUIDANCE: &str = "Review with DBA: SELECT on system.processes, system.query_log, system.parts, system.merges, system.replicas, system.events, system.metrics and system.asynchronous_metrics; REMOTE for cluster scope. query_log is server configuration. OpenRows never flushes logs, changes settings or grants access.";
const COVERAGE: &str = "Bounded best-effort observations under readonly, time, read and result caps; server uptime decreases reset counter baselines. No blocking chains, waits or index usage statistics exist in ClickHouse.";
/// Profile option naming a cluster for all-replica activity; absent means node scope.
pub(super) const CLUSTER_OPTION: &str = "monitor_cluster";

pub(super) async fn discover(session: &mut ClickHouseSession) -> Vec<Descriptor> {
    session.monitor_config = None;
    let settings = match guard::query(session, SETTINGS, 1_000).await {
        Ok(result) => result.rows.into_iter().next().unwrap_or_default(),
        Err(error) => return missing(&failure(&error, Source::Database).state),
    };
    let cell = |index: usize| settings.get(index).cloned().flatten().unwrap_or_default();
    if !supported(&cell(0)) {
        return missing(&unavailable(
            "Supported layouts: ClickHouse 25.3 LTS or newer",
        ));
    }
    let (cluster, scope) = cluster(session, &cell(1)).await;
    let mut config = Config {
        cluster,
        counters: [false; 3],
        descriptors: Vec::new(),
        epoch: counters::Epoch::default(),
    };
    let denied = probe_counters(session, &mut config).await;
    let sampled = cell(2).parse::<f64>().is_ok_and(|p| p < 1.0);
    for source in Source::ALL {
        let mut state = initial(source, &config, denied.clone());
        if state == SampleState::Ok {
            state = probe(session, source, &config).await;
        }
        let mut descriptor = descriptor(source, state);
        coverage(&mut descriptor, &scope, sampled, config.counters);
        config.descriptors.push(descriptor);
    }
    let descriptors = config.descriptors.clone();
    session.monitor_config = Some(config);
    descriptors
}
pub(super) fn supported(version: &str) -> bool {
    let mut parts = version.split('.').map(str::parse::<u32>);
    match (parts.next(), parts.next()) {
        (Some(Ok(major)), Some(Ok(minor))) => (major, minor) >= (25, 3),
        _ => false,
    }
}
/// Cluster scope needs a valid name, an existing cluster and REMOTE access.
async fn cluster(session: &ClickHouseSession, host: &str) -> (Option<String>, String) {
    let node = format!("Node scope: the connected node {host} only.");
    let Some(requested) = session.client.config.driver_options.get(CLUSTER_OPTION) else {
        return (None, node);
    };
    let Some(name) = sources::cluster_name(requested) else {
        return (
            None,
            format!("{node} The requested cluster name is invalid."),
        );
    };
    let sql = format!("SELECT count() FROM clusterAllReplicas('{name}', system.one)");
    match guard::query(session, &sql, 1_000).await {
        Ok(_) => (
            Some(name.to_owned()),
            format!(
                "Cluster scope for running queries: all replicas of {name} via clusterAllReplicas; other sources stay on node {host}."
            ),
        ),
        Err(_) => (
            None,
            format!(
                "{node} Cluster {name} is missing or clusterAllReplicas/REMOTE is not granted."
            ),
        ),
    }
}
/// Readable counter tables; a denied table is named, the others still feed counters.
async fn probe_counters(session: &ClickHouseSession, config: &mut Config) -> Option<SampleState> {
    let mut denied = None;
    for index in 0..3 {
        let Some(sql) = counters::probe(index) else {
            continue;
        };
        match guard::query(session, &sql, 1_000).await {
            Ok(_) => config.counters[index] = true,
            Err(error) => denied = Some(failure(&error, Source::Database).state),
        }
    }
    denied
}
fn initial(source: Source, config: &Config, denied: Option<SampleState>) -> SampleState {
    match source {
        Source::Indexes => unavailable(
            "Not applicable: ClickHouse has no index usage statistics; data-skipping indexes are in the schema view",
        ),
        Source::Database if !config.counters.contains(&true) => {
            denied.unwrap_or(SampleState::Unknown)
        }
        _ => SampleState::Ok,
    }
}
async fn probe(session: &ClickHouseSession, source: Source, config: &Config) -> SampleState {
    let (sql, _) = sources::query(source, config);
    match guard::query(session, &sql, 1_000).await {
        Ok(_) => SampleState::Ok,
        Err(error) => failure(&error, source).state,
    }
}
fn descriptor(source: Source, state: SampleState) -> Descriptor {
    let scope = if matches!(source, Source::Tables | Source::Statements) {
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
fn coverage(descriptor: &mut Descriptor, scope: &str, sampled: bool, counters: [bool; 3]) {
    let note = match descriptor.source {
        Source::Activity => {
            " Running queries from system.processes; visibility follows the user's grants and row policies. Memory tracking can omit some allocations."
        }
        Source::Database => {
            " Node counters from system.events/metrics/asynchronous_metrics include collector queries; a denied table leaves its counters unknown."
        }
        Source::Statements => {
            " Top-N covers 200 normalised query hashes from QueryFinish/ExceptionWhileProcessing events in the selected database during the current hour; the hourly window start resets baselines. Log flushing delays recent queries; initial and secondary (shard) queries are separate rows."
        }
        Source::Tables => {
            " Active parts per table in the selected database; many parts per partition slows merges and inserts."
        }
        Source::Io => " In-progress merges and mutations on this node.",
        Source::Replication => {
            " Replicated tables only; queue and absolute delay are per table on this node."
        }
        Source::Indexes => "",
    };
    descriptor.coverage = format!("{scope} {}{note}", descriptor.coverage);
    if sampled && descriptor.source == Source::Statements {
        descriptor
            .coverage
            .push_str(" log_queries_probability < 1: the query log is sampled.");
    }
    if descriptor.source == Source::Database {
        for (readable, table) in counters.iter().zip(counters::TABLES) {
            if !readable {
                descriptor.coverage.push(' ');
                descriptor.coverage.push_str(table.1);
                descriptor.coverage.push_str(" is not readable.");
            }
        }
    }
}
fn missing(state: &SampleState) -> Vec<Descriptor> {
    Source::ALL
        .into_iter()
        .map(|s| descriptor(s, state.clone()))
        .collect()
}
