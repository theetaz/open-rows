// SPDX-License-Identifier: Apache-2.0
//! In-app incidents and explicit background collection; no OS daemon or external delivery.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command inputs are owned"
)]
use super::{MonitorTarget, registry::Key};
use crate::connections::{Connections, IpcError};
use openrows_monitor::alerts::{AlertEngine, AlertRule, Incident, Observation, Transition};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{
    collections::BTreeMap,
    sync::{Mutex, PoisonError},
};
use tauri::{Manager, State};
use tauri_specta::Event;

/// Incident state survives pause/restart for the app run, bounded by target count.
const TARGETS: usize = 16;

/// Per-target engines; evaluation happens only on behalf of a running, unlocked collector.
#[derive(Default)]
pub(crate) struct Alerts {
    engines: Mutex<BTreeMap<Key, AlertEngine>>,
}
impl Alerts {
    /// Apply the latest observations; a locked app does no evaluation at all.
    pub(super) fn observe(
        &self,
        key: &Key,
        observations: &[Observation],
        unlocked: bool,
    ) -> Vec<Transition> {
        if !unlocked || observations.is_empty() {
            return Vec::new();
        }
        let mut engines = self.engines.lock().unwrap_or_else(PoisonError::into_inner);
        if !engines.contains_key(key) && !admit(&mut engines, key) {
            return Vec::new();
        }
        let Some(engine) = engines.get_mut(key) else {
            return Vec::new();
        };
        observations
            .iter()
            .flat_map(|observation| engine.observe(observation))
            .collect()
    }
    fn read(&self, key: &Key) -> (Vec<AlertRule>, Vec<Incident>) {
        let engines = self.engines.lock().unwrap_or_else(PoisonError::into_inner);
        engines.get(key).map_or_else(
            || (openrows_monitor::alerts::default_rules(), Vec::new()),
            |engine| (engine.rules().to_vec(), engine.incidents()),
        )
    }
}

/// Create an engine for a new target, evicting one without open incidents when full.
fn admit(engines: &mut BTreeMap<Key, AlertEngine>, key: &Key) -> bool {
    if engines.len() >= TARGETS {
        let idle = engines
            .iter()
            .find(|(_, engine)| quiet(engine))
            .map(|(key, _)| key.clone());
        let Some(idle) = idle else {
            return false;
        };
        engines.remove(&idle);
    }
    let Ok(fresh) = AlertEngine::new(openrows_monitor::alerts::default_rules()) else {
        return false;
    };
    engines.insert(key.clone(), fresh);
    true
}
fn quiet(engine: &AlertEngine) -> bool {
    engine
        .incidents()
        .iter()
        .all(|incident| incident.ended_ms.is_some())
}

/// Rules, retained incidents and the background opt-in for one target.
#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IncidentsReply {
    rules: Vec<AlertRule>,
    incidents: Vec<Incident>,
    background: bool,
}
/// In-app notification of an opened or resolved incident.
#[derive(Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MonitorIncident {
    target: MonitorTarget,
    transition: Transition,
}
impl Event for MonitorIncident {
    const NAME: &'static str = "monitor-incident";
}

/// Read deterministic rules and incidents; no collector is started.
#[tauri::command]
#[specta::specta]
pub(crate) fn monitor_incidents(
    state: State<'_, Connections>,
    target: MonitorTarget,
) -> Result<IncidentsReply, IpcError> {
    state
        .app
        .state::<crate::security::Security>()
        .admit("monitor_incidents")?;
    let key = target.key();
    let (rules, incidents) = state.monitors.alerts.read(&key);
    Ok(IncidentsReply {
        rules,
        incidents,
        background: state.monitors.background(&key),
    })
}

/// Opt a running collector in or out of background monitoring while the app is open.
#[tauri::command]
#[specta::specta]
pub(crate) fn monitor_background(
    state: State<'_, Connections>,
    target: MonitorTarget,
    enabled: bool,
) -> Result<(), IpcError> {
    state
        .app
        .state::<crate::security::Security>()
        .admit("monitor_background")?;
    state
        .monitors
        .set_background(&target.key(), enabled)
        .map_err(super::commands::error)
}

/// Evaluate one collector tick and notify in-app; skipped entirely while locked.
pub(super) fn evaluate(
    app: &tauri::AppHandle,
    target: &MonitorTarget,
    monitor: &openrows_monitor::Monitor,
) {
    let unlocked = app
        .state::<crate::security::Security>()
        .admit("monitor_alerts")
        .is_ok();
    let observations = if unlocked {
        monitor.observations()
    } else {
        Vec::new()
    };
    let transitions =
        app.state::<Connections>()
            .monitors
            .alerts
            .observe(&target.key(), &observations, unlocked);
    for transition in transitions {
        let _ = MonitorIncident {
            target: target.clone(),
            transition,
        }
        .emit(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openrows_monitor::{SampleState, Scope};
    fn waiting(seconds: u32) -> Observation {
        Observation {
            metric: "sessions:waiting".into(),
            scope: Scope::Server,
            wall_ms: f64::from(seconds) * 1_000.0,
            value: Some(9.0),
            state: SampleState::Ok,
        }
    }
    #[test]
    fn locked_or_empty_evaluation_does_no_work() {
        let alerts = Alerts::default();
        let key: Key = ("p".into(), "db".into());
        let samples: Vec<_> = (0..10).map(|second| waiting(second * 5)).collect();
        assert!(alerts.observe(&key, &samples, false).is_empty());
        assert!(alerts.observe(&key, &[], true).is_empty());
        assert!(
            alerts
                .engines
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .is_empty()
        );
        let opened = alerts.observe(&key, &samples, true);
        assert!(matches!(opened.as_slice(), [Transition::Opened(_)]));
        assert_eq!(alerts.read(&key).1.len(), 1);
        // Locking keeps incidents but never re-evaluates them.
        let healthy: Vec<_> = (10..20)
            .map(|second| Observation {
                value: Some(0.0),
                ..waiting(second * 5)
            })
            .collect();
        assert!(alerts.observe(&key, &healthy, false).is_empty());
        assert_eq!(alerts.read(&key).1[0].ended_ms, None);
    }
}
