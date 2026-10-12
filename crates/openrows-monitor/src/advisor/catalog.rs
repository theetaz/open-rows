// SPDX-License-Identifier: Apache-2.0
//! Fixed catalog projections retain names and signatures, never predicate literals.
use super::{AdvisorInput, Coverage, IndexEvidence, LockEvidence, TableEvidence};
use crate::{LiveStore, Reading, SampleState, Source};
use std::collections::BTreeMap;

/// Bounded consecutive identity observations, invalidated by gaps and reset epochs.
#[derive(Default)]
pub(crate) struct Observations(BTreeMap<Source, (String, BTreeMap<String, u32>)>);
impl Observations {
    /// Update only complete table/activity observations from the current epoch.
    pub(crate) fn record(&mut self, source: Source, reading: &Reading) {
        if !matches!(source, Source::Tables | Source::Activity) {
            return;
        }
        if reading.state != SampleState::Ok || reading.partial {
            self.0.remove(&source);
            return;
        }
        let (epoch, counts) = self.0.entry(source).or_default();
        let mut previous = std::mem::take(counts);
        if *epoch != reading.epoch {
            previous.clear();
        }
        for row in reading.rows.iter().take(200) {
            let id = if source == Source::Tables {
                cell(reading, row, "relation_id").map(str::to_owned)
            } else {
                lock_identity(reading, row)
            };
            if let Some(id) = id {
                let count = previous.get(&id).copied().unwrap_or(0);
                counts.insert(id, count.saturating_add(1));
            }
        }
        epoch.clone_from(&reading.epoch);
    }
    /// Conservative owned-memory charge within the shared live-store budget.
    pub(crate) fn bytes(&self) -> usize {
        self.0
            .values()
            .map(|(epoch, counts)| {
                epoch.capacity() + counts.keys().map(|k| k.capacity() + 128).sum::<usize>()
            })
            .sum()
    }
    fn count(&self, source: Source, id: &str) -> u32 {
        self.0
            .get(&source)
            .and_then(|(_, counts)| counts.get(id))
            .copied()
            .unwrap_or(0)
    }
}

/// Find a named cell without treating absent columns or values as zero.
pub(crate) fn cell<'a>(
    reading: &Reading,
    row: &'a [Option<String>],
    name: &str,
) -> Option<&'a str> {
    let index = reading
        .columns
        .iter()
        .position(|column| column.name == name)?;
    row.get(index)?.as_deref()
}
fn number(reading: &Reading, row: &[Option<String>], name: &str) -> Option<f64> {
    cell(reading, row, name)?
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0)
}
fn table(reading: &Reading, row: &[Option<String>]) -> Option<TableEvidence> {
    Some(TableEvidence {
        id: cell(reading, row, "relation_id")?.into(),
        schema: cell(reading, row, "schema")?.into(),
        name: cell(reading, row, "table")?.into(),
        estimated_rows: number(reading, row, "estimated_rows"),
        size_bytes: number(reading, row, "size_bytes"),
        live_tuples: number(reading, row, "live_tuples"),
        dead_tuples: number(reading, row, "dead_tuples"),
        modifications: number(reading, row, "modifications"),
        analyze_age_ms: number(reading, row, "analyze_age_ms"),
        vacuum_age_ms: number(reading, row, "vacuum_age_ms"),
        samples: 0,
    })
}
fn index(reading: &Reading, row: &[Option<String>]) -> Option<IndexEvidence> {
    let keys = serde_json::from_str::<Vec<String>>(cell(reading, row, "key_columns")?).ok()?;
    let usable = match cell(reading, row, "usable") {
        Some("t") => true,
        Some("f") => false,
        _ => return None,
    };
    Some(IndexEvidence {
        id: cell(reading, row, "index_id")?.into(),
        table_id: cell(reading, row, "relation_id")?.into(),
        schema: cell(reading, row, "schema")?.into(),
        name: cell(reading, row, "index")?.into(),
        keys,
        signature: cell(reading, row, "definition_key")?.into(),
        usable,
        protected: cell(reading, row, "protected") != Some("f"),
        scans: number(reading, row, "idx_scan"),
        reset_age_ms: number(reading, row, "reset_age_ms"),
    })
}

impl LiveStore {
    /// Derive typed advisor inputs only from permitted current source readings.
    #[must_use]
    pub fn advisor_input(&self, now: f64) -> AdvisorInput {
        let tables = self.tables.get(&Source::Tables);
        let indexes = self.tables.get(&Source::Indexes);
        let activity = self.tables.get(&Source::Activity);
        let (table_values, tables_complete) = decoded(tables, table);
        let (index_values, indexes_complete) = decoded(indexes, index);
        let mut tables_coverage = self.coverage(Source::Tables);
        tables_coverage.complete &= tables_complete;
        let mut indexes_coverage = self.coverage(Source::Indexes);
        indexes_coverage.complete &= indexes_complete;
        AdvisorInput {
            now_ms: now,
            tables_coverage,
            indexes_coverage,
            activity_coverage: self.coverage(Source::Activity),
            locks: rows(activity)
                .iter()
                .filter_map(|row| {
                    let reading = activity?;
                    let identity = lock_identity(reading, row)?;
                    Some(LockEvidence {
                        samples: self.observations.count(Source::Activity, &identity),
                        identity,
                        pid: cell(reading, row, "pid")?.into(),
                        blockers: cell(reading, row, "blockers")?.into(),
                        wait: cell(reading, row, "wait")?.into(),
                        transaction_ms: number(reading, row, "transaction_ms"),
                    })
                })
                .collect(),
            tables: table_values
                .into_iter()
                .map(|mut t| {
                    t.samples = self.observations.count(Source::Tables, &t.id);
                    t
                })
                .collect(),
            indexes: index_values,
            plans: self.plans.clone(),
            intervals: self
                .statement_history
                .intervals
                .values()
                .flat_map(|p| p.iter().cloned())
                .collect(),
        }
    }
    fn coverage(&self, source: Source) -> Coverage {
        Coverage {
            collected_ms: self.table_walls.get(&source).copied().unwrap_or(0.0),
            complete: self
                .tables
                .get(&source)
                .is_some_and(|r| r.state == SampleState::Ok && !r.partial),
        }
    }
    /// Store only the bounded literal-free projection of an explicitly requested plan.
    pub fn record_plan(&mut self, plan: super::PlanEvidence) {
        self.plans
            .retain(|old| old.statement_id != plan.statement_id);
        if self.plans.len() >= 8 {
            self.plans.remove(0);
        }
        self.plans.push(plan);
    }
}

fn rows(source: Option<&Reading>) -> &[Vec<Option<String>>] {
    source
        .filter(|r| r.state == SampleState::Ok)
        .map(|r| r.rows.as_slice())
        .unwrap_or_default()
}

fn decoded<T>(
    source: Option<&Reading>,
    decode: fn(&Reading, &[Option<String>]) -> Option<T>,
) -> (Vec<T>, bool) {
    let input = rows(source);
    let values: Vec<_> = input
        .iter()
        .filter_map(|row| decode(source?, row))
        .collect();
    let complete = values.len() == input.len();
    (values, complete)
}

fn lock_identity(reading: &Reading, row: &[Option<String>]) -> Option<String> {
    if cell(reading, row, "state") != Some("active") {
        return None;
    }
    let blockers = cell(reading, row, "blockers")?;
    if blockers.trim_matches(['{', '}']).is_empty()
        || !cell(reading, row, "wait")?.starts_with("Lock")
    {
        return None;
    }
    Some(format!(
        "{}:{}:{}",
        cell(reading, row, "pid")?,
        cell(reading, row, "query_start")?,
        blockers.trim_matches(['{', '}'])
    ))
}
