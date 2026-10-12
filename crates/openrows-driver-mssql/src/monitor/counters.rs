// SPDX-License-Identifier: Apache-2.0
//! Whitelisted `sys.dm_os_performance_counters` resource evidence as one
//! server row; absent counters stay NULL (unknown), never zero.
use std::fmt::Write as _;

/// Object (after the instance prefix), counter, optional instance, metric
/// name and unit; `counter` units are cumulative per-second counters.
const COUNTERS: [(&str, &str, Option<&str>, &str, &str); 11] = [
    (
        "SQL Statistics",
        "Batch Requests/sec",
        None,
        "batch_requests",
        "counter",
    ),
    (
        "SQL Statistics",
        "SQL Compilations/sec",
        None,
        "compilations",
        "counter",
    ),
    (
        "SQL Statistics",
        "SQL Re-Compilations/sec",
        None,
        "recompilations",
        "counter",
    ),
    (
        "General Statistics",
        "User Connections",
        None,
        "user_connections",
        "count",
    ),
    (
        "General Statistics",
        "Processes blocked",
        None,
        "processes_blocked",
        "count",
    ),
    (
        "Locks",
        "Lock Waits/sec",
        Some("_Total"),
        "lock_waits",
        "counter",
    ),
    (
        "Locks",
        "Number of Deadlocks/sec",
        Some("_Total"),
        "deadlocks",
        "counter",
    ),
    (
        "Buffer Manager",
        "Page life expectancy",
        None,
        "page_life_expectancy",
        "s",
    ),
    (
        "Memory Manager",
        "Total Server Memory (KB)",
        None,
        "total_server_memory_kb",
        "KB",
    ),
    (
        "Memory Manager",
        "Target Server Memory (KB)",
        None,
        "target_server_memory_kb",
        "KB",
    ),
    (
        "Memory Manager",
        "Memory Grants Pending",
        None,
        "memory_grants_pending",
        "count",
    ),
];

/// Raw-fraction counters divided by their base at read time, as fractions.
const RATIOS: [(&str, &str, Option<&str>, &str); 2] = [
    (
        "Buffer Manager",
        "Buffer cache hit ratio",
        None,
        "buffer_cache_hit_ratio",
    ),
    (
        "Resource Pool Stats",
        "CPU usage %",
        Some("default"),
        "cpu_usage",
    ),
];

fn matches(object: &str, counter: &str, instance: Option<&str>) -> String {
    let instance = instance.map_or_else(String::new, |i| format!(" AND instance_name = N'{i}'"));
    format!("RTRIM(object_name) LIKE N'%:{object}' AND counter_name = N'{counter}'{instance}")
}

/// Pivot of the whitelisted counters, followed by the epoch.
pub(super) fn query() -> String {
    let mut cells = vec!["'server'".to_owned()];
    for (object, counter, instance, _, _) in COUNTERS {
        let condition = matches(object, counter, instance);
        cells.push(format!("MAX(CASE WHEN {condition} THEN cntr_value END)"));
    }
    for (object, counter, instance, _) in RATIOS {
        let value = matches(object, counter, instance);
        let base = matches(object, &format!("{counter} base"), instance);
        cells.push(format!(
            "1.0 * MAX(CASE WHEN {value} THEN cntr_value END) / NULLIF(MAX(CASE WHEN {base} THEN cntr_value END), 0)"
        ));
    }
    cells.push("@epoch".into());
    // Prefiltering on the exact names keeps the pivot to a few rows.
    let names: Vec<_> =
        COUNTERS
            .iter()
            .map(|(_, counter, ..)| format!("N'{counter}'"))
            .chain(RATIOS.iter().flat_map(|(_, counter, ..)| {
                [format!("N'{counter}'"), format!("N'{counter} base'")]
            }))
            .collect();
    format!(
        "SELECT {} FROM sys.dm_os_performance_counters WHERE counter_name IN ({})",
        cells.join(", "),
        names.join(", ")
    )
}

/// Column spec matching [`query`] without the epoch.
pub(super) fn spec() -> String {
    let mut spec = String::from("server_id:text");
    let units = COUNTERS
        .iter()
        .map(|(_, _, _, name, unit)| (*name, *unit))
        .chain(RATIOS.iter().map(|(_, _, _, name)| (*name, "fraction")));
    for (name, unit) in units {
        let _ = write!(spec, ",{name}:{unit}");
    }
    spec
}
