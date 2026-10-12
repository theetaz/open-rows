// SPDX-License-Identifier: Apache-2.0
//! Deterministic, model-free incidents over live aggregates (docs/28 §3).
mod rules;
#[cfg(test)]
mod tests;
use crate::{SampleState, Scope};
pub use rules::{AlertRule, Comparison, Hold, RuleError, Severity, default_rules};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const INCIDENTS: usize = 50;
const ONSET: usize = 8;

/// One scalar observation; anything but a finite `Ok` value is unknown.
#[derive(Clone, Debug, PartialEq)]
pub struct Observation {
    /// Live aggregate name.
    pub metric: String,
    /// Server or selected-database scope.
    pub scope: Scope,
    /// Wall-clock collection time; non-increasing times are ignored.
    pub wall_ms: f64,
    /// Measured value, absent when unknown.
    pub value: Option<f64>,
    /// Sample validity.
    pub state: SampleState,
}

impl Observation {
    fn known(&self) -> Option<f64> {
        self.value
            .filter(|value| self.state == SampleState::Ok && value.is_finite())
    }
}

/// A value recorded as incident evidence.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    /// Collection time.
    pub wall_ms: f64,
    /// Observed value.
    pub value: f64,
}

/// An open or resolved incident with onset/recovery evidence.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Incident {
    /// Engine-local monotonic identity.
    pub id: u32,
    /// Rule snapshot at onset, including thresholds.
    pub rule: AlertRule,
    /// Deduplication scope.
    pub scope: Scope,
    /// Wall time of the first breaching sample of the firing streak.
    pub started_ms: f64,
    /// Wall time of the sample that completed recovery.
    pub ended_ms: Option<f64>,
    /// The breaching streak that opened the incident (at most eight values).
    pub onset: Vec<Evidence>,
    /// Worst observed value while open.
    pub peak: Evidence,
    /// Latest known value while open, or the recovery value.
    pub last: Evidence,
}

/// Lifecycle change for in-app notification.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(tag = "kind", content = "incident", rename_all = "camelCase")]
pub enum Transition {
    /// Breach held for the required duration.
    Opened(Incident),
    /// Recovery held for the required duration.
    Resolved(Incident),
}

#[derive(Default)]
struct Track {
    last_wall: Option<f64>,
    streak: Vec<Evidence>,
    since: Option<(f64, u32)>,
    open: Option<u32>,
    resolved_ms: Option<f64>,
}

impl Track {
    /// Unknown data interrupts both breach and recovery streaks; nothing changes state.
    fn interrupt(&mut self) {
        self.streak.clear();
        self.since = None;
    }
    fn extend(&mut self, point: Evidence) -> (f64, u32) {
        let (start, count) = self.since.get_or_insert((point.wall_ms, 0));
        *count = count.saturating_add(1);
        (*start, *count)
    }
}

/// Per-target incident state; no collection, timers or I/O of its own.
pub struct AlertEngine {
    rules: Vec<AlertRule>,
    tracks: BTreeMap<(String, Scope), Track>,
    incidents: VecDeque<Incident>,
    next: u32,
}

impl AlertEngine {
    /// Validate every rule and reject duplicate identities.
    /// # Errors
    /// The first invalid rule.
    pub fn new(rules: Vec<AlertRule>) -> Result<Self, RuleError> {
        let mut ids = BTreeSet::new();
        for rule in &rules {
            rule.validate()?;
            if !ids.insert(rule.id.as_str()) {
                return Err(RuleError::Duplicate);
            }
        }
        Ok(Self {
            rules,
            tracks: BTreeMap::new(),
            incidents: VecDeque::new(),
            next: 1,
        })
    }
    /// Active rules.
    #[must_use]
    pub fn rules(&self) -> &[AlertRule] {
        &self.rules
    }
    /// Retained incidents, newest first (at most fifty).
    #[must_use]
    pub fn incidents(&self) -> Vec<Incident> {
        self.incidents.iter().rev().cloned().collect()
    }
    /// Apply one observation to every matching rule.
    pub fn observe(&mut self, observation: &Observation) -> Vec<Transition> {
        let matching: Vec<_> = (0..self.rules.len())
            .filter(|index| self.rules[*index].metric == observation.metric)
            .collect();
        matching
            .into_iter()
            .filter_map(|index| self.step(index, observation))
            .collect()
    }

    fn step(&mut self, index: usize, observation: &Observation) -> Option<Transition> {
        let rule = &self.rules[index];
        let track = self
            .tracks
            .entry((rule.id.clone(), observation.scope))
            .or_default();
        if track
            .last_wall
            .is_some_and(|last| observation.wall_ms <= last)
        {
            return None;
        }
        track.last_wall = Some(observation.wall_ms);
        let Some(value) = observation.known() else {
            track.interrupt();
            return None;
        };
        let point = Evidence {
            wall_ms: observation.wall_ms,
            value,
        };
        let open = track.open;
        match open {
            Some(id) => self.recover(index, observation.scope, id, point),
            None => self.fire(index, observation.scope, point),
        }
    }

    fn fire(&mut self, index: usize, scope: Scope, point: Evidence) -> Option<Transition> {
        let rule = &self.rules[index];
        let track = self.tracks.get_mut(&(rule.id.clone(), scope))?;
        if !rule.breached(point.value) {
            track.interrupt();
            return None;
        }
        let (start, count) = track.extend(point);
        if track.streak.len() < ONSET {
            track.streak.push(point);
        }
        let held = held(rule.fire_after, start, count, point.wall_ms);
        let cooled = track
            .resolved_ms
            .is_none_or(|resolved| point.wall_ms - resolved >= f64::from(rule.cooldown_ms));
        if !held || !cooled {
            return None;
        }
        let incident = Incident {
            id: self.next,
            rule: rule.clone(),
            scope,
            started_ms: start,
            ended_ms: None,
            onset: std::mem::take(&mut track.streak),
            peak: point,
            last: point,
        };
        track.open = Some(self.next);
        track.since = None;
        self.next = self.next.wrapping_add(1).max(1);
        self.retain(incident.clone());
        Some(Transition::Opened(incident))
    }

    fn recover(
        &mut self,
        index: usize,
        scope: Scope,
        id: u32,
        point: Evidence,
    ) -> Option<Transition> {
        let rule = &self.rules[index];
        let worse = |a: f64, b: f64| match rule.comparison {
            Comparison::Above => a > b,
            Comparison::Below => a < b,
        };
        let incident = self.incidents.iter_mut().find(|incident| incident.id == id);
        let track = self.tracks.get_mut(&(rule.id.clone(), scope))?;
        let Some(incident) = incident else {
            // Evicted from the bounded list: forget the stale open marker.
            track.open = None;
            return None;
        };
        incident.last = point;
        if worse(point.value, incident.peak.value) {
            incident.peak = point;
        }
        if !rule.healthy(point.value) {
            track.since = None;
            return None;
        }
        let (start, count) = track.extend(point);
        if !held(rule.clear_after, start, count, point.wall_ms) {
            return None;
        }
        incident.ended_ms = Some(point.wall_ms);
        track.open = None;
        track.since = None;
        track.resolved_ms = Some(point.wall_ms);
        Some(Transition::Resolved(incident.clone()))
    }

    fn retain(&mut self, incident: Incident) {
        if self.incidents.len() >= INCIDENTS {
            let resolved = self
                .incidents
                .iter()
                .position(|incident| incident.ended_ms.is_some());
            self.incidents.remove(resolved.unwrap_or(0));
        }
        self.incidents.push_back(incident);
    }
}

fn held(hold: Hold, start: f64, count: u32, now: f64) -> bool {
    count >= hold.samples && now - start >= f64::from(hold.ms)
}
