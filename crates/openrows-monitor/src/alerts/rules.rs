// SPDX-License-Identifier: Apache-2.0
//! Validated deterministic alert rules and the built-in catalogue.
use serde::{Deserialize, Serialize};

/// Direction in which a metric becomes unhealthy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Comparison {
    /// Fires at or above `fire`; healthy at or below `clear`.
    Above,
    /// Fires at or below `fire`; healthy at or above `clear`.
    Below,
}

/// Incident severity shown in the Monitor; no external delivery.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    /// Degraded but serving.
    Warning,
    /// Needs prompt attention.
    Critical,
}

/// Consecutive known samples and elapsed wall time that must both be met.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Hold {
    /// At least one consecutive known sample.
    pub samples: u32,
    /// Milliseconds from the first to the latest sample of the streak.
    pub ms: u32,
}

/// A threshold rule with hysteresis, duration, recovery and cooldown.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AlertRule {
    /// Stable identity; deduplication key together with scope.
    pub id: String,
    /// Short label.
    pub title: String,
    /// Live aggregate name, e.g. `sessions:waiting`.
    pub metric: String,
    /// Unhealthy direction.
    pub comparison: Comparison,
    /// Firing threshold.
    pub fire: f64,
    /// Separate recovery threshold (hysteresis band between the two).
    pub clear: f64,
    /// How long the breach must hold before an incident opens.
    pub fire_after: Hold,
    /// How long recovery must hold before the incident resolves.
    pub clear_after: Hold,
    /// Minimum quiet time after recovery before the same rule/scope reopens.
    pub cooldown_ms: u32,
    /// Displayed severity.
    pub severity: Severity,
}

/// Rejected rule definition.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RuleError {
    /// Missing identity, title or metric.
    #[error("Alert rule needs an id, title and metric")]
    Empty,
    /// Duplicate rule identity.
    #[error("Alert rule ids must be unique")]
    Duplicate,
    /// NaN or infinite thresholds.
    #[error("Alert thresholds must be finite numbers")]
    NonFinite,
    /// Recovery threshold is not on the healthy side of the firing threshold.
    #[error("Recovery threshold must be on the healthy side of the firing threshold")]
    Hysteresis,
    /// Zero samples or durations/cooldowns above one hour.
    #[error("Alert durations need 1–1,000 samples and at most one hour")]
    Hold,
}

const HOUR_MS: u32 = 3_600_000;

impl Hold {
    fn valid(self) -> bool {
        (1..=1_000).contains(&self.samples) && self.ms <= HOUR_MS
    }
}

impl AlertRule {
    /// Check identity, finiteness, hysteresis direction and bounds.
    /// # Errors
    /// The first violated constraint.
    pub fn validate(&self) -> Result<(), RuleError> {
        if self.id.is_empty() || self.title.is_empty() || self.metric.is_empty() {
            return Err(RuleError::Empty);
        }
        if !self.fire.is_finite() || !self.clear.is_finite() {
            return Err(RuleError::NonFinite);
        }
        let ordered = match self.comparison {
            Comparison::Above => self.clear < self.fire,
            Comparison::Below => self.clear > self.fire,
        };
        if !ordered {
            return Err(RuleError::Hysteresis);
        }
        if !self.fire_after.valid() || !self.clear_after.valid() || self.cooldown_ms > HOUR_MS {
            return Err(RuleError::Hold);
        }
        Ok(())
    }
    /// The value is in the firing region.
    #[must_use]
    pub fn breached(&self, value: f64) -> bool {
        match self.comparison {
            Comparison::Above => value >= self.fire,
            Comparison::Below => value <= self.fire,
        }
    }
    /// The value is in the recovery region (outside the hysteresis band).
    #[must_use]
    pub fn healthy(&self, value: f64) -> bool {
        match self.comparison {
            Comparison::Above => value <= self.clear,
            Comparison::Below => value >= self.clear,
        }
    }
}

struct Preset(
    &'static str,
    &'static str,
    &'static str,
    Comparison,
    f64,
    f64,
);

const PRESETS: [Preset; 7] = [
    Preset(
        "waiting-sessions",
        "Waiting sessions",
        "sessions:waiting",
        Comparison::Above,
        5.0,
        1.0,
    ),
    Preset(
        "idle-in-transaction",
        "Idle in transaction",
        "sessions:idle in transaction",
        Comparison::Above,
        10.0,
        5.0,
    ),
    Preset(
        "active-sessions",
        "Active sessions",
        "sessions:active",
        Comparison::Above,
        50.0,
        40.0,
    ),
    Preset(
        "cache-hit-ratio",
        "Cache hit ratio",
        "Database:cache_hit_ratio",
        Comparison::Below,
        0.90,
        0.95,
    ),
    Preset(
        "buffer-pool-hit-ratio",
        "Buffer pool hit ratio",
        "Database:buffer_pool_hit_ratio",
        Comparison::Below,
        0.90,
        0.95,
    ),
    Preset(
        "deadlocks",
        "Deadlocks",
        "Database:deadlocks",
        Comparison::Above,
        0.01,
        0.0,
    ),
    Preset(
        "replay-lag",
        "Replication replay lag",
        "Replication:replay_lag_ms",
        Comparison::Above,
        30_000.0,
        10_000.0,
    ),
];

/// Built-in rules; deadlocks fire on the first observed deadlock interval.
#[must_use]
pub fn default_rules() -> Vec<AlertRule> {
    PRESETS
        .iter()
        .map(|Preset(id, title, metric, comparison, fire, clear)| {
            let instant = *id == "deadlocks";
            let critical = instant || *id == "replay-lag";
            AlertRule {
                id: (*id).into(),
                title: (*title).into(),
                metric: (*metric).into(),
                comparison: *comparison,
                fire: *fire,
                clear: *clear,
                fire_after: Hold {
                    samples: if instant { 1 } else { 3 },
                    ms: if instant { 0 } else { 30_000 },
                },
                clear_after: Hold {
                    samples: 3,
                    ms: 30_000,
                },
                cooldown_ms: 60_000,
                severity: if critical {
                    Severity::Critical
                } else {
                    Severity::Warning
                },
            }
        })
        .collect()
}
