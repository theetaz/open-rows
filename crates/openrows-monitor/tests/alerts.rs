// SPDX-License-Identifier: Apache-2.0
//! Live aggregates feed deterministic rules; a stopped generation yields no work.
use openrows_monitor::alerts::{AlertEngine, Hold, Transition, default_rules};
use openrows_monitor::{
    Column, Descriptor, MetricKind, Monitor, MonitorReader, ReadFuture, Reading, SampleState,
    Scope, Source,
};
use std::time::Duration;

struct Waiting;
impl MonitorReader for Waiting {
    fn discover(&mut self) -> ReadFuture<'_, Vec<Descriptor>> {
        Box::pin(async { Vec::new() })
    }
    fn read(&mut self, _: Source, _: u32) -> ReadFuture<'_, Reading> {
        let text = |name: &str| Column {
            name: name.into(),
            unit: "text".into(),
            kind: MetricKind::Text,
        };
        let row = |pid: &str| vec![Some(pid.into()), Some("active".into()), Some("Lock".into())];
        Box::pin(async move {
            Reading {
                terminal: false,
                state: SampleState::Ok,
                epoch: "e".into(),
                columns: vec![text("pid"), text("state"), text("wait_event")],
                rows: (1..=6).map(|pid| row(&pid.to_string())).collect(),
                partial: false,
            }
        })
    }
    fn cancellation(&self) -> ReadFuture<'static, ()> {
        Box::pin(async {})
    }
}

#[tokio::test(start_paused = true)]
async fn live_waiting_sessions_open_an_incident_and_stop_ends_observation()
-> Result<(), Box<dyn std::error::Error>> {
    let descriptor = Descriptor {
        source: Source::Activity,
        state: SampleState::Ok,
        scope: Scope::Server,
        interval_ms: 2_000,
        guidance: None,
        coverage: "test".into(),
    };
    let monitor = Monitor::spawn(Box::new(Waiting), vec![descriptor], 2_000)?;
    // Paused test time does not advance the wall clock, so hold by samples only.
    let rules = default_rules()
        .into_iter()
        .map(|mut rule| {
            rule.fire_after = Hold { samples: 3, ms: 0 };
            rule
        })
        .collect();
    let mut engine = AlertEngine::new(rules)?;
    let mut opened = Vec::new();
    for _ in 0..40 {
        tokio::time::advance(Duration::from_millis(2_000)).await;
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        for observation in monitor.observations() {
            opened.extend(engine.observe(&observation));
        }
    }
    let waiting = monitor
        .observations()
        .into_iter()
        .find(|observation| observation.metric == "sessions:waiting")
        .ok_or("missing waiting sessions")?;
    assert_eq!(waiting.value, Some(6.0));
    assert_eq!(waiting.scope, Scope::Server);
    assert!(
        matches!(opened.as_slice(), [Transition::Opened(incident)] if incident.rule.id == "waiting-sessions")
    );
    monitor.stop();
    assert!(monitor.observations().is_empty());
    Ok(())
}
