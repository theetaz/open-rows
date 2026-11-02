// SPDX-License-Identifier: Apache-2.0
//! Monitor history retention: tiered downsampling on write, 24-hour expiry and the
//! retained-row caps. Every function runs inside its caller's transaction.
use super::{StoreError, error::sqlite};
use rusqlite::{Connection, params};

pub(super) const DAY_MS: i64 = 86_400_000;
const MINUTE_MS: i64 = 60_000;
const HOUR_MS: i64 = 60 * MINUTE_MS;

/// `(age, width)`: samples older than `age` roll up into `width` buckets. Each width
/// stays at or below the read bucket of every history range that reads that age
/// (1 h view reads 2 h in 24 s buckets, 6 h reads 12 h in 144 s, 24 h in 288 s), so
/// charts keep one point per read bucket and draw no false gaps.
pub(super) const TIERS: [(i64, i64); 3] = [
    (10 * MINUTE_MS, 20_000),
    (2 * HOUR_MS, 2 * MINUTE_MS),
    (12 * HOUR_MS, 4 * MINUTE_MS),
];

/// Samples older than this are already rolled up and are not written raw again.
pub(super) fn raw_cutoff(now: i64) -> i64 {
    let (age, width) = TIERS[0];
    aligned(now.saturating_sub(age), width)
}

fn aligned(time: i64, width: i64) -> i64 {
    time.div_euclid(width).saturating_mul(width)
}

/// Roll every finer row older than each tier's aligned cutoff into that tier, coarsest
/// first so a long pause moves raw samples straight to the coarsest bucket.
pub(super) fn downsample(conn: &Connection, now: i64) -> Result<(), StoreError> {
    for (index, &(age, width)) in TIERS.iter().enumerate().rev() {
        let finer: Vec<String> = std::iter::once(0)
            .chain(TIERS[..index].iter().map(|&(_, finer)| finer))
            .map(|span| span.to_string())
            .collect();
        let finer = finer.join(", ");
        let cutoff = aligned(now.saturating_sub(age), width);
        conn.prepare_cached(&rollup_sql(&finer))
            .and_then(|mut statement| statement.execute(params![width, cutoff]))
            .map_err(sqlite("downsample monitor history"))?;
        conn.prepare_cached(&format!(
            "DELETE FROM monitor_history WHERE span_ms IN ({finer}) AND collected_ms < ?1"
        ))
        .and_then(|mut statement| statement.execute([cutoff]))
        .map_err(sqlite("downsample monitor history"))?;
    }
    Ok(())
}

/// One bucket per scope and metric: valid-value extrema, sum and count, the last valid
/// value and the latest missing state (a gap in a bucket is never hidden). A source row
/// that shares the bucket's start time is replaced; an existing bucket of the same or a
/// coarser width (only after the clock moved back) is merged.
fn rollup_sql(finer: &str) -> String {
    format!(
        "INSERT INTO monitor_history (profile_id, database_name, metric, collected_ms, value, state, span_ms, min_value, max_value, sum_value, ok_count) \
         SELECT profile_id, database_name, metric, bucket, last_value, bucket_state, ?1, min(min_value), max(max_value), sum(sum_value), sum(ok_count) \
         FROM (SELECT *, collected_ms - collected_ms % ?1 AS bucket, \
           first_value(value) OVER (PARTITION BY profile_id, database_name, metric, collected_ms - collected_ms % ?1 ORDER BY value IS NULL, collected_ms DESC) AS last_value, \
           first_value(state) OVER (PARTITION BY profile_id, database_name, metric, collected_ms - collected_ms % ?1 ORDER BY state = 'ok', collected_ms DESC) AS bucket_state \
           FROM monitor_history WHERE span_ms IN ({finer}) AND collected_ms < ?2) \
         WHERE true GROUP BY profile_id, database_name, metric, bucket \
         ON CONFLICT (profile_id, database_name, metric, collected_ms) DO UPDATE SET \
           value = CASE WHEN span_ms < excluded.span_ms THEN excluded.value ELSE coalesce(excluded.value, value) END, \
           state = CASE WHEN span_ms < excluded.span_ms OR excluded.state != 'ok' THEN excluded.state ELSE state END, \
           min_value = CASE WHEN span_ms < excluded.span_ms THEN excluded.min_value ELSE min(coalesce(min_value, excluded.min_value), coalesce(excluded.min_value, min_value)) END, \
           max_value = CASE WHEN span_ms < excluded.span_ms THEN excluded.max_value ELSE max(coalesce(max_value, excluded.max_value), coalesce(excluded.max_value, max_value)) END, \
           sum_value = CASE WHEN span_ms < excluded.span_ms THEN excluded.sum_value ELSE sum_value + excluded.sum_value END, \
           ok_count = CASE WHEN span_ms < excluded.span_ms THEN excluded.ok_count ELSE ok_count + excluded.ok_count END, \
           span_ms = max(span_ms, excluded.span_ms)"
    )
}

/// Expire rows older than a day, then evict the oldest rows above the per-connection
/// and app-wide caps; downsampling keeps a day well below them at default cadence.
pub(super) fn enforce(
    conn: &Connection,
    now: i64,
    connection_rows: i64,
    app_rows: i64,
) -> Result<(), StoreError> {
    conn.execute(
        "DELETE FROM monitor_history WHERE collected_ms < ?1",
        [now.saturating_sub(DAY_MS)],
    )
    .map_err(sqlite("expire monitor history"))?;
    if conn.query_row("SELECT coalesce(max(total), 0) FROM (SELECT count(*) AS total FROM monitor_history GROUP BY profile_id)", [], |row| row.get::<_, i64>(0)).map_err(sqlite("count connection history"))? > connection_rows {
    conn.execute("DELETE FROM monitor_history WHERE (profile_id, database_name, metric, collected_ms) IN (SELECT profile_id, database_name, metric, collected_ms FROM (SELECT *, row_number() OVER (PARTITION BY profile_id ORDER BY collected_ms DESC, database_name, metric) AS position FROM monitor_history) WHERE position > ?1)", [connection_rows])
        .map_err(sqlite("cap connection history"))?;
    }
    if conn
        .query_row("SELECT count(*) FROM monitor_history", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(sqlite("count app history"))?
        <= app_rows
    {
        return Ok(());
    }
    conn.execute("DELETE FROM monitor_history WHERE (profile_id, database_name, metric, collected_ms) IN (SELECT profile_id, database_name, metric, collected_ms FROM monitor_history ORDER BY collected_ms DESC, profile_id, database_name, metric LIMIT -1 OFFSET ?1)", [app_rows])
        .map(drop).map_err(sqlite("cap app history"))
}

pub(super) fn expire_startup(conn: &Connection) -> Result<(), StoreError> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map_err(|_| StoreError::Corrupt {
            column: "monitor clock",
        })?;
    let now = i64::try_from(duration.as_millis()).map_err(|_| StoreError::Corrupt {
        column: "monitor clock",
    })?;
    conn.execute(
        "DELETE FROM monitor_history WHERE collected_ms < ?1",
        [now.saturating_sub(DAY_MS)],
    )
    .map(drop)
    .map_err(sqlite("expire monitor history on startup"))
}
