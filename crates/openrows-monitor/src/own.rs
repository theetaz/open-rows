// SPDX-License-Identifier: Apache-2.0
//! Own-session statement ledger for embedded and remote-libSQL targets: only
//! statements this process executed are visible, never other applications.
use crate::{Column, MetricKind, Reading, SampleState};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, Weak};

/// Statement shapes retained per target; the cheapest shape is evicted first.
const IDENTITIES: usize = 200;

/// Engine step counters of one execution, where the engine reports them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EngineSteps {
    /// `SQLITE_STMTSTATUS_FULLSCAN_STEP`.
    pub fullscan_steps: u64,
    /// `SQLITE_STMTSTATUS_SORT`.
    pub sorts: u64,
    /// `SQLITE_STMTSTATUS_VM_STEP`.
    pub vm_steps: u64,
}

/// One finished own execution, measured by the client.
#[derive(Clone, Copy, Debug, Default)]
pub struct Execution {
    /// Client wall time in milliseconds.
    pub elapsed_ms: f64,
    /// Rows returned to the app.
    pub rows: u64,
    /// The execution ended in an error.
    pub failed: bool,
    /// Engine counters; `None` when the transport does not expose them.
    pub steps: Option<EngineSteps>,
}

/// Cumulative own-request totals since the ledger epoch.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Totals {
    /// Finished executions.
    pub requests: u64,
    /// Executions that ended in an error.
    pub errors: u64,
    /// Summed client wall time in milliseconds.
    pub total_ms: f64,
    /// Rows returned.
    pub rows: u64,
}

impl Totals {
    /// Counter columns appended to a Database reading.
    #[must_use]
    pub fn columns() -> Vec<Column> {
        [
            ("requests", "count"),
            ("request_errors", "count"),
            ("request_ms", "ms"),
            ("rows_returned", "count"),
        ]
        .into_iter()
        .map(|(name, unit)| Column {
            name: name.into(),
            unit: unit.into(),
            kind: MetricKind::Counter,
        })
        .collect()
    }

    /// Cells matching [`Totals::columns`].
    #[must_use]
    pub fn cells(&self) -> Vec<Option<String>> {
        vec![
            Some(self.requests.to_string()),
            Some(self.errors.to_string()),
            Some(format!("{:.3}", self.total_ms)),
            Some(self.rows.to_string()),
        ]
    }
}

struct Entry {
    text: Option<String>,
    totals: Totals,
    max_ms: f64,
    steps: Option<EngineSteps>,
}

#[derive(Default)]
struct Entries {
    statements: HashMap<String, Entry>,
    totals: Totals,
    evicted: bool,
}

/// Process-wide own executions against one target, shared by its sessions.
pub struct Ledger {
    epoch: String,
    entries: Mutex<Entries>,
}

type Registry = Mutex<HashMap<String, Weak<Ledger>>>;

impl Ledger {
    /// The live ledger for `key` (a file path or remote URL), created on
    /// first use; it ends when the last session holding it closes.
    #[must_use]
    pub fn shared(key: &str) -> Arc<Self> {
        static REGISTRY: OnceLock<Registry> = OnceLock::new();
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let mut registry = REGISTRY
            .get_or_init(Registry::default)
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        registry.retain(|_, ledger| ledger.strong_count() > 0);
        if let Some(ledger) = registry.get(key).and_then(Weak::upgrade) {
            return ledger;
        }
        let ledger = Arc::new(Self {
            epoch: format!("own-{}", SEQUENCE.fetch_add(1, Ordering::Relaxed)),
            entries: Mutex::default(),
        });
        registry.insert(key.to_owned(), Arc::downgrade(&ledger));
        ledger
    }

    /// Counter epoch; a new ledger resets every own counter.
    #[must_use]
    pub fn epoch(&self) -> &str {
        &self.epoch
    }

    /// Record one execution under its literal-free statement shape.
    pub fn record(&self, sql: &str, execution: Execution) {
        let text = openrows_sql::normalize_monitor_query(sql);
        let id = text.as_deref().map_or_else(|| utility_id(sql), fingerprint);
        let mut entries = self.lock();
        add(&mut entries.totals, &execution);
        if !entries.statements.contains_key(&id) && entries.statements.len() >= IDENTITIES {
            evict(&mut entries);
        }
        let entry = entries.statements.entry(id).or_insert_with(|| Entry {
            text,
            totals: Totals::default(),
            max_ms: 0.0,
            steps: None,
        });
        add(&mut entry.totals, &execution);
        entry.max_ms = entry.max_ms.max(execution.elapsed_ms);
        if let Some(steps) = execution.steps {
            let sum = entry.steps.get_or_insert_with(EngineSteps::default);
            sum.fullscan_steps = sum.fullscan_steps.saturating_add(steps.fullscan_steps);
            sum.sorts = sum.sorts.saturating_add(steps.sorts);
            sum.vm_steps = sum.vm_steps.saturating_add(steps.vm_steps);
        }
    }

    /// Totals across every own execution since the epoch.
    #[must_use]
    pub fn totals(&self) -> Totals {
        self.lock().totals
    }

    /// Top statement shapes by total own wall time, as a Statements reading.
    #[must_use]
    pub fn statements(&self) -> Reading {
        let entries = self.lock();
        let mut ranked: Vec<_> = entries.statements.iter().collect();
        ranked.sort_by(|a, b| b.1.totals.total_ms.total_cmp(&a.1.totals.total_ms));
        Reading {
            terminal: false,
            state: SampleState::Ok,
            epoch: self.epoch.clone(),
            columns: statement_columns(),
            rows: ranked
                .into_iter()
                .map(|(id, e)| statement_row(id, e))
                .collect(),
            partial: entries.evicted,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Entries> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn add(totals: &mut Totals, execution: &Execution) {
    totals.requests = totals.requests.saturating_add(1);
    totals.errors = totals.errors.saturating_add(u64::from(execution.failed));
    totals.rows = totals.rows.saturating_add(execution.rows);
    if execution.elapsed_ms.is_finite() && execution.elapsed_ms >= 0.0 {
        totals.total_ms += execution.elapsed_ms;
    }
}

/// Drop the shape with the least total time; its counters restart if seen again.
fn evict(entries: &mut Entries) {
    let cheapest = entries
        .statements
        .iter()
        .min_by(|a, b| a.1.totals.total_ms.total_cmp(&b.1.totals.total_ms))
        .map(|(id, _)| id.clone());
    if let Some(id) = cheapest {
        entries.statements.remove(&id);
        entries.evicted = true;
    }
}

/// Stable FNV-1a identity of the normalised text; never derived from literals.
fn fingerprint(text: &str) -> String {
    let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    });
    format!("{hash:016x}")
}

/// Utility or unparsable statements group by their leading keyword only.
fn utility_id(sql: &str) -> String {
    let verb: String = sql
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .chars()
        .take_while(char::is_ascii_alphabetic)
        .take(16)
        .collect();
    format!("other:{}", verb.to_ascii_uppercase())
}

fn statement_columns() -> Vec<Column> {
    [
        ("statement_id", "text", MetricKind::Text),
        ("calls", "count", MetricKind::Counter),
        ("total_ms", "ms", MetricKind::Counter),
        ("cumulative_mean_ms", "ms", MetricKind::Gauge),
        ("max_since_reset_ms", "ms", MetricKind::Gauge),
        ("rows", "count", MetricKind::Counter),
        ("errors", "count", MetricKind::Counter),
        ("fullscan_steps", "count", MetricKind::Counter),
        ("sorts", "count", MetricKind::Counter),
        ("vm_steps", "count", MetricKind::Counter),
        ("query", "query", MetricKind::Query),
    ]
    .into_iter()
    .map(|(name, unit, kind)| Column {
        name: name.into(),
        unit: unit.into(),
        kind,
    })
    .collect()
}

#[expect(
    clippy::cast_precision_loss,
    reason = "call counts stay far below 2^52 within one app run"
)]
fn statement_row(id: &str, entry: &Entry) -> Vec<Option<String>> {
    let totals = entry.totals;
    let mean = (totals.requests > 0).then(|| totals.total_ms / totals.requests as f64);
    let step = |pick: fn(&EngineSteps) -> u64| entry.steps.as_ref().map(|s| pick(s).to_string());
    vec![
        Some(id.to_owned()),
        Some(totals.requests.to_string()),
        Some(format!("{:.3}", totals.total_ms)),
        mean.map(|value| format!("{value:.3}")),
        Some(format!("{:.3}", entry.max_ms)),
        Some(totals.rows.to_string()),
        Some(totals.errors.to_string()),
        step(|s| s.fullscan_steps),
        step(|s| s.sorts),
        step(|s| s.vm_steps),
        entry.text.clone(),
    ]
}
