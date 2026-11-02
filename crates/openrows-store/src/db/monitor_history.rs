// SPDX-License-Identifier: Apache-2.0
//! Fixed, default-off scalar history; SQL text and source rows cannot enter this store.
use super::monitor_allowlist::{known, validate};
use super::monitor_rollup::{DAY_MS, downsample, enforce, raw_cutoff};
use super::{LocalStore, StoreError, error::sqlite};
use rusqlite::params;
use serde::{Deserialize, Serialize};

pub(super) use super::monitor_rollup::expire_startup;

// A conservative 2-KiB charge covers bounded keys, both indexes and page overhead.
const CONNECTION_ROWS: i64 = 50 * 1_024 * 1_024 / 2_048;
const APP_ROWS: i64 = 100 * 1_024 * 1_024 / 2_048;

/// One aggregate observation, without statement text, identities or result rows.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorAggregate {
    /// Allowlisted scalar metric name (database identities are removed).
    pub metric: String,
    /// Wall-clock sample time in milliseconds.
    pub collected_ms: i64,
    /// A real finite value only when the state is ok.
    pub value: Option<f64>,
    /// Small validity label, without server error details.
    pub state: String,
}

/// One read bucket of a metric. Values are present only when every stored sample in the
/// bucket was valid; otherwise `state` names the latest missing state and all are `None`.
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorBucket {
    /// Allowlisted scalar metric name.
    pub metric: String,
    /// Start of the read bucket in wall-clock milliseconds.
    pub collected_ms: i64,
    /// Last valid value in the bucket.
    pub value: Option<f64>,
    /// `ok`, or the latest missing state in the bucket.
    pub state: String,
    /// Smallest valid value in the bucket, including rolled-up samples.
    pub min: Option<f64>,
    /// Largest valid value in the bucket, including rolled-up samples.
    pub max: Option<f64>,
    /// Mean of every valid sample in the bucket, weighted by rolled-up sample counts.
    pub mean: Option<f64>,
}

impl LocalStore {
    /// Replace the explicit opt-in list and purge disabled profiles atomically.
    /// # Errors
    /// Storage failure; invalid keys are refused.
    pub fn set_monitor_profiles(&self, profiles: &[String], now: i64) -> Result<(), StoreError> {
        if profiles.len() > 1_000 || profiles.iter().any(|id| id.len() > 64) {
            return Err(StoreError::Corrupt {
                column: "monitor profile",
            });
        }
        let mut conn = self.lock()?;
        let tx = conn
            .transaction()
            .map_err(sqlite("configure monitor history"))?;
        tx.execute("DELETE FROM monitor_profiles", [])
            .map_err(sqlite("disable history"))?;
        for profile in profiles {
            tx.execute(
                "INSERT OR IGNORE INTO monitor_profiles VALUES (?1)",
                [profile],
            )
            .map_err(sqlite("enable history"))?;
        }
        tx.execute("DELETE FROM monitor_history WHERE profile_id NOT IN (SELECT profile_id FROM monitor_profiles)", [])
            .map_err(sqlite("purge disabled history"))?;
        enforce(&tx, now, CONNECTION_ROWS, APP_ROWS)?;
        tx.commit().map_err(sqlite("configure monitor history"))
    }

    /// Record scalar samples only while this profile is opted in, then downsample, expire
    /// and cap in the same transaction. Unknown metric names are skipped before the write
    /// and counted in the returned total; samples already rolled up are not written again.
    /// # Errors
    /// Invalid scope, state, value or time, or storage failure; the whole batch rolls back.
    pub fn record_monitor(
        &self,
        scope: (&str, &str),
        points: &[MonitorAggregate],
        now: i64,
    ) -> Result<usize, StoreError> {
        if scope.0.len() > 64 || scope.1.len() > 256 || points.len() > 64 {
            return Err(StoreError::Corrupt {
                column: "monitor scope",
            });
        }
        let (known, unknown): (Vec<_>, Vec<_>) =
            points.iter().partition(|point| known(&point.metric));
        for point in &known {
            validate(point, now)?;
        }
        let mut conn = self.lock()?;
        let tx = conn
            .transaction()
            .map_err(sqlite("record monitor history"))?;
        for point in known {
            tx.prepare_cached("INSERT OR IGNORE INTO monitor_history (profile_id, database_name, metric, collected_ms, value, state, span_ms, min_value, max_value, sum_value, ok_count) SELECT ?1, ?2, ?3, ?4, ?5, ?6, 0, ?5, ?5, coalesce(?5, 0), ?5 IS NOT NULL WHERE ?4 >= ?7 AND EXISTS (SELECT 1 FROM monitor_profiles WHERE profile_id = ?1)")
                .and_then(|mut statement| statement.execute(params![scope.0, scope.1, point.metric, point.collected_ms, point.value, point.state, raw_cutoff(now)]))
                .map_err(sqlite("record monitor aggregate"))?;
        }
        downsample(&tx, now)?;
        enforce(&tx, now, CONNECTION_ROWS, APP_ROWS)?;
        tx.commit().map_err(sqlite("record monitor history"))?;
        Ok(unknown.len())
    }

    /// Read at most 300 buckets per metric over raw and rolled-up rows, preserving missing
    /// states within each bucket.
    /// # Errors
    /// Storage failure; ranges are clamped to the retained 24 hours.
    pub fn monitor_history(
        &self,
        scope: (&str, &str),
        range: (i64, i64),
    ) -> Result<Vec<MonitorBucket>, StoreError> {
        let conn = self.lock()?;
        enforce(&conn, range.1, CONNECTION_ROWS, APP_ROWS)?;
        let since = range.0.max(range.1.saturating_sub(DAY_MS));
        let bucket = (range.1.saturating_sub(since) / 300).saturating_add(1);
        let mut statement = conn.prepare_cached("SELECT metric, ?3 + ((collected_ms - ?3) / ?5) * ?5, value, state, low, high, total / weight FROM (SELECT metric, collected_ms, value, state, min(min_value) OVER bucket AS low, max(max_value) OVER bucket AS high, sum(sum_value) OVER bucket AS total, sum(ok_count) OVER bucket AS weight, row_number() OVER (PARTITION BY metric, (collected_ms - ?3) / ?5 ORDER BY state != 'ok' DESC, collected_ms DESC) AS position FROM monitor_history WHERE profile_id = ?1 AND database_name = ?2 AND collected_ms >= ?3 AND collected_ms <= ?4 WINDOW bucket AS (PARTITION BY metric, (collected_ms - ?3) / ?5)) WHERE position = 1 ORDER BY metric, collected_ms LIMIT 19200")
            .map_err(sqlite("read monitor history"))?;
        statement
            .query_map(params![scope.0, scope.1, since, range.1, bucket], |row| {
                let state: String = row.get(3)?;
                let valid = |index| -> rusqlite::Result<Option<f64>> {
                    Ok(row.get::<_, Option<f64>>(index)?.filter(|_| state == "ok"))
                };
                Ok(MonitorBucket {
                    metric: row.get(0)?,
                    collected_ms: row.get(1)?,
                    value: valid(2)?,
                    min: valid(4)?,
                    max: valid(5)?,
                    mean: valid(6)?,
                    state,
                })
            })
            .map_err(sqlite("read monitor history"))?
            .collect::<Result<_, _>>()
            .map_err(sqlite("decode monitor history"))
    }

    /// Purge one connection or all connections without changing their opt-in preference.
    /// # Errors
    /// Storage failure.
    pub fn purge_monitor_history(&self, profile: Option<&str>) -> Result<(), StoreError> {
        self.lock()?
            .execute(
                "DELETE FROM monitor_history WHERE ?1 IS NULL OR profile_id = ?1",
                [profile],
            )
            .map(drop)
            .map_err(sqlite("purge monitor history"))
    }
}

#[cfg(test)]
#[path = "monitor_history_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "monitor_rollup_tests.rs"]
mod rollup_tests;
