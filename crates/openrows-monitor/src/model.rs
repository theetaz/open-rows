// SPDX-License-Identifier: Apache-2.0
//! Source contracts and explicit missing-data semantics.
use serde::{Deserialize, Serialize};

/// Reviewed engine statistics source, never user-supplied SQL.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// Server client sessions and bounded blocking edges.
    Activity,
    /// Selected database cumulative counters.
    Database,
    /// Table scans and maintenance estimates.
    Tables,
    /// Index scan and tuple counters.
    Indexes,
    /// Server sender/receiver replication state.
    Replication,
    /// Optional statement counters, without sampled text.
    Statements,
    /// Optional server I/O counters on PG 16+.
    Io,
}

impl Source {
    /// All sources, in stable scheduling order.
    pub const ALL: [Self; 7] = [
        Self::Activity,
        Self::Database,
        Self::Tables,
        Self::Indexes,
        Self::Replication,
        Self::Statements,
        Self::Io,
    ];
    /// Minimum accepted cadence in milliseconds.
    #[must_use]
    pub fn floor_ms(self, live_ms: u32) -> u32 {
        match self {
            Self::Activity => live_ms,
            Self::Database | Self::Statements | Self::Io => live_ms.max(15_000),
            Self::Tables | Self::Indexes | Self::Replication => 60_000,
        }
    }
}

/// Observation validity; absent values never become a numeric zero.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(tag = "kind", content = "reason", rename_all = "camelCase")]
pub enum SampleState {
    /// A real observation, including a valid zero.
    Ok,
    /// First baseline or a NULL server value.
    Unknown,
    /// Source cannot supply this measurement.
    Unavailable(String),
    /// Access was denied or other-user visibility is restricted.
    PermissionDenied(String),
    /// Missed interval, timeout, pause or lost session.
    Gap,
    /// Changed reset epoch, generation or decreasing counter.
    CounterReset,
}

/// Measurement scope retained alongside every source.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    /// All visible sessions on this server.
    Server,
    /// The selected database only.
    Database,
}

/// Readability, cadence and reviewed enabling guidance.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Descriptor {
    /// Engine-owned source.
    pub source: Source,
    /// Effective read visibility.
    pub state: SampleState,
    /// Scope of the observations.
    pub scope: Scope,
    /// Minimum poll cadence.
    pub interval_ms: u32,
    /// Minimum grants or settings, offered for review only.
    pub guidance: Option<String>,
    /// Version/permission requirements and continuity limitations.
    pub coverage: String,
}

/// Numeric semantics used for reset-aware analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetricKind {
    /// Instantaneous numeric measurement.
    Gauge,
    /// Cumulative counter; converted to interval rates.
    Counter,
    /// Opaque identity or label.
    Text,
    /// Raw SQL, normalised in core before storage or IPC.
    Query,
}

/// A projected source column with documented units.
#[derive(Clone, Debug)]
pub struct Column {
    /// Stable column/metric name.
    pub name: String,
    /// Native source unit, e.g. ms, bytes, count.
    pub unit: String,
    /// Gauge/counter/text semantics.
    pub kind: MetricKind,
}

/// Bounded source read; raw query cells must pass core sanitisation.
#[derive(Clone, Debug)]
pub struct Reading {
    /// Protocol loss/client timeout requires explicit restart with a fresh session.
    pub terminal: bool,
    /// Source validity and restriction.
    pub state: SampleState,
    /// Server start/reset marker, absent means best-effort continuity.
    pub epoch: String,
    /// Explicit projected columns; first column is scoped identity.
    pub columns: Vec<Column>,
    /// Up to 1,000 rows and 1 MiB text, admitted before allocation by driver.
    pub rows: Vec<Vec<Option<String>>>,
    /// Row cap or incomplete source coverage.
    pub partial: bool,
}

impl Reading {
    /// A failed or disabled source without fabricated rows.
    #[must_use]
    pub fn missing(state: SampleState) -> Self {
        Self {
            terminal: false,
            state,
            epoch: String::new(),
            columns: Vec::new(),
            rows: Vec::new(),
            partial: false,
        }
    }
}

/// Small status event/snapshot; data windows travel as Arrow bytes.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SourceStatus {
    /// Source being reported.
    pub source: Source,
    /// Last observation state.
    pub state: SampleState,
    /// Wall-clock collection time in milliseconds.
    pub collected_ms: f64,
    /// Last successful collection time, for freshness.
    pub last_success_ms: Option<f64>,
    /// Adaptive cadence visible to the user.
    pub interval_ms: u32,
    /// Last bounded statement duration.
    pub duration_ms: f64,
    /// Monotonic generation-local sequence.
    pub sequence: u32,
    /// Truncation or eviction is visible.
    pub partial: bool,
}

/// Privacy-safe, typed monitor failure without server text.
#[derive(Clone, Debug, thiserror::Error)]
pub enum MonitorError {
    /// No collector exists for this connection/database.
    #[error("Monitor is stopped; explicitly start it again")]
    Stopped,
    /// Global admission or sample cap was reached.
    #[error("Monitoring resource limit reached")]
    Limit,
    /// A data window cannot be encoded.
    #[error("Monitoring window encoding failed")]
    Encoding,
    /// Unsupported engine or cadence.
    #[error("Monitoring is unavailable for this engine or interval")]
    Unsupported,
}

/// One bounded Arrow page; scalar history and source tables share the same path.
#[derive(Clone, Copy, Debug)]
pub struct WindowOptions {
    /// None selects live scalar series.
    pub source: Option<Source>,
    /// Offset into the stable snapshot, for pagination.
    pub row_start: u32,
    /// At most 1,000 rows (100 statements).
    pub row_count: u32,
    /// Statement ranking by covered live-window weighted mean instead of total.
    pub mean: bool,
}
