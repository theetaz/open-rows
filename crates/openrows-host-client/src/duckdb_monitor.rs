// SPDX-License-Identifier: Apache-2.0
//! `DuckDB` diagnostics contract (ADR-0011): cheap catalog functions and
//! settings on the owning session, plus own statement totals. `DuckDB` has no
//! process list, lock view or replication, so those sources are unavailable.
use openrows_monitor::{
    Column, Descriptor, MetricKind, Reading, SampleState, Scope, Source, Totals,
};

/// The ADR-0011 boundary shown beside every `DuckDB` panel.
pub(crate) const OWN: &str = "Own OpenRows session only — other processes are not visible.";

/// One bounded row: block usage, WAL, memory, temp files and settings.
pub(crate) const STATS: &str = "SELECT current_database(), \
d.block_size::VARCHAR, d.total_blocks::VARCHAR, d.used_blocks::VARCHAR, \
d.free_blocks::VARCHAR, d.wal_size, \
(SELECT sum(memory_usage_bytes) FROM duckdb_memory())::VARCHAR, \
current_setting('memory_limit')::VARCHAR, \
(SELECT count(*) FROM duckdb_temporary_files())::VARCHAR, \
(SELECT coalesce(sum(size), 0) FROM duckdb_temporary_files())::VARCHAR, \
current_setting('threads')::VARCHAR \
FROM (SELECT 1) AS one \
LEFT JOIN pragma_database_size() AS d ON d.database_name = current_database()";

/// Gauge columns after `database_id`, in [`STATS`] order (byte text parsed).
const GAUGES: [(&str, &str); 10] = [
    ("block_size", "bytes"),
    ("total_blocks", "count"),
    ("used_blocks", "count"),
    ("free_blocks", "count"),
    ("wal_bytes", "bytes"),
    ("memory_bytes", "bytes"),
    ("memory_limit_bytes", "bytes"),
    ("temp_files", "count"),
    ("temp_bytes", "bytes"),
    ("threads", "count"),
];

/// Fixed descriptors; discovery runs no query, so it never waits on the user.
#[must_use]
pub fn duckdb_monitor_descriptors() -> Vec<Descriptor> {
    Source::ALL
        .into_iter()
        .map(|source| Descriptor {
            source,
            state: match source {
                Source::Database | Source::Statements => SampleState::Ok,
                other => unavailable(other),
            },
            scope: Scope::Database,
            interval_ms: source.floor_ms(5_000),
            guidance: None,
            coverage: coverage(source),
        })
        .collect()
}

fn coverage(source: Source) -> String {
    match source {
        Source::Database => "pragma_database_size block usage, WAL size, duckdb_memory(), memory_limit, duckdb_temporary_files() and threads, read on the owning session after the running statement, plus own request totals. A busy session is a gap.".into(),
        Source::Statements => format!(
            "{OWN} Client wall time, rows and errors per statement shape since this app opened the database."
        ),
        _ => OWN.to_owned(),
    }
}

/// Why a source is absent for `DuckDB`.
pub(crate) fn unavailable(source: Source) -> SampleState {
    let reason = match source {
        Source::Activity => "DuckDB has no server process list or lock view",
        Source::Replication => "DuckDB files have no replication",
        Source::Tables | Source::Indexes => {
            "Per-table/index statistics need storage scans, which are not run on a tick"
        }
        Source::Io => "DuckDB exposes no I/O or wait counters",
        Source::Database | Source::Statements => "Unavailable",
    };
    SampleState::Unavailable(format!("{reason}. {OWN}"))
}

/// A Database reading from one [`STATS`] row; missing values stay unknown.
pub(crate) fn reading(row: Option<&[Option<String>]>, totals: Totals, epoch: &str) -> Reading {
    let cell = |at: usize| row.and_then(|row| row.get(at)).cloned().flatten();
    let mut columns = vec![column("database_id", "text", MetricKind::Text)];
    columns.push(column("database_bytes", "bytes", MetricKind::Gauge));
    columns.extend(
        GAUGES
            .iter()
            .map(|(name, unit)| column(name, unit, MetricKind::Gauge)),
    );
    columns.extend(Totals::columns());
    let integer = |at: usize| cell(at).and_then(|text| text.trim().parse::<u64>().ok());
    let size = integer(1)
        .zip(integer(2))
        .and_then(|(block, total)| block.checked_mul(total));
    let mut values = vec![
        Some(cell(0).unwrap_or_else(|| "main".into())),
        size.map(|v| v.to_string()),
    ];
    for (offset, (_, unit)) in GAUGES.iter().enumerate() {
        let text = cell(offset + 1);
        values.push(if *unit == "bytes" {
            text.as_deref().and_then(bytes).map(|v| v.to_string())
        } else {
            text.filter(|text| text.trim().parse::<u64>().is_ok())
        });
    }
    values.extend(totals.cells());
    Reading {
        terminal: false,
        state: SampleState::Ok,
        epoch: epoch.to_owned(),
        columns,
        rows: vec![values],
        partial: false,
    }
}

fn column(name: &str, unit: &str, kind: MetricKind) -> Column {
    Column {
        name: name.into(),
        unit: unit.into(),
        kind,
    }
}

/// Bytes from a plain integer or `DuckDB`'s formatted size (`1.5 GiB`, `0 bytes`).
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a finite, non-negative size below 2^64 bytes"
)]
pub(crate) fn bytes(text: &str) -> Option<u64> {
    let text = text.trim();
    let split = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let number: f64 = number.parse().ok()?;
    let scale: f64 = match unit.trim().to_ascii_lowercase().as_str() {
        "" | "b" | "byte" | "bytes" => 1.0,
        "kb" => 1e3,
        "mb" => 1e6,
        "gb" => 1e9,
        "tb" => 1e12,
        "kib" => 1024.0,
        "mib" => 1024.0 * 1024.0,
        "gib" => 1024.0 * 1024.0 * 1024.0,
        "tib" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    let value = (number * scale).round();
    (0.0..1.8e19).contains(&value).then_some(value as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatted_sizes_parse_to_bytes() {
        assert_eq!(bytes("0 bytes"), Some(0));
        assert_eq!(bytes("512"), Some(512));
        assert_eq!(bytes("1.5 KiB"), Some(1536));
        assert_eq!(bytes("2.0 GB"), Some(2_000_000_000));
        assert_eq!(bytes("unlimited"), None);
        assert_eq!(bytes("-1 bytes"), None);
    }

    #[test]
    fn a_missing_row_keeps_values_unknown() {
        let reading = reading(None, Totals::default(), "own-0");
        let row = &reading.rows[0];
        assert_eq!(row[0].as_deref(), Some("main"));
        assert!(row[1..=GAUGES.len() + 1].iter().all(Option::is_none));
        assert_eq!(reading.columns.len(), row.len());
    }

    #[test]
    fn database_size_is_blocks_times_block_size() {
        let cells: Vec<_> = [
            "main", "262144", "4", "3", "1", "1.0 KiB", "2048", "1.0 GiB", "0", "0", "8",
        ]
        .into_iter()
        .map(|cell| Some(cell.to_owned()))
        .collect();
        let reading = reading(Some(&cells), Totals::default(), "own-0");
        let value = |name: &str| {
            let at = reading.columns.iter().position(|c| c.name == name)?;
            reading.rows[0][at].clone()
        };
        assert_eq!(value("database_bytes").as_deref(), Some("1048576"));
        assert_eq!(value("wal_bytes").as_deref(), Some("1024"));
        assert_eq!(value("memory_limit_bytes").as_deref(), Some("1073741824"));
        assert_eq!(value("threads").as_deref(), Some("8"));
    }
}
