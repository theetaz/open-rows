// SPDX-License-Identifier: Apache-2.0
//! Read existing monitor/store evidence after `ToolExecutor`'s telemetry admission.
use crate::connections::Connections;
use openrows_ai::{
    investigation,
    sharing::{ContextBundle, EvidenceState},
};
use openrows_mcp::{McpError, ToolInput};
use tauri::Manager;
impl super::backend::Backend {
    pub(super) async fn monitoring(
        &self,
        tool: &str,
        input: &ToolInput,
    ) -> Result<ContextBundle, McpError> {
        let id = input.connection.as_deref().ok_or(McpError::Invalid)?;
        let database = input.database.as_deref().ok_or(McpError::Invalid)?;
        let profile = crate::connections::monitor::profile_key_for_history(id);
        let key = (profile, database.to_owned());
        let now = crate::queries::history::now_ms();
        let state = self.0.state::<Connections>();
        let mut plan = Vec::new();
        let mut evidence = if tool == "read_metric_history" {
            let app = self.0.clone();
            let selected = key.clone();
            tokio::task::spawn_blocking(move || history(&app, &selected, now))
                .await
                .map_err(|_| McpError::Database)??
        } else {
            let monitor = state.monitors.get(&key).map_err(|_| McpError::Database)?;
            let binding = format!("{}:{}:{}", key.0, key.1, monitor.generation());
            let data = monitor
                .advisor_input(milliseconds(now))
                .map_err(|_| McpError::Database)?;
            match tool {
                "read_health_snapshot" => {
                    let mut points = investigation::health(
                        &binding,
                        &monitor.snapshot(),
                        &monitor.capabilities(),
                        milliseconds(now),
                    );
                    points.extend(
                        monitor
                            .observations()
                            .iter()
                            .take(100)
                            .map(|point| investigation::observation(&binding, point)),
                    );

                    points
                }
                "read_query_statistics" => investigation::queries(&binding, &data),
                "read_lock_graph" => investigation::locks(&binding, &data),
                "read_monitor_plans" => {
                    plan = investigation::plan_context(&data);
                    investigation::plans(&binding, &data)
                }
                _ => return Err(McpError::Invalid),
            }
        };
        if evidence.is_empty() {
            let mut gap = investigation::sample(
                &format!("{}:{}", key.0, key.1),
                "telemetry:missing",
                milliseconds(now),
                None,
            );
            gap.state = EvidenceState::Unavailable;
            evidence.push(gap);
        }
        evidence.truncate(200);
        Ok(ContextBundle {
            plan,
            evidence,
            ..ContextBundle::default()
        })
    }
}
#[expect(
    clippy::cast_precision_loss,
    reason = "epoch milliseconds fit f64 exactly"
)]
fn milliseconds(time: i64) -> f64 {
    time as f64
}
fn history(
    app: &tauri::AppHandle,
    key: &(String, String),
    now: i64,
) -> Result<Vec<openrows_ai::sharing::Evidence>, McpError> {
    let points = app
        .state::<openrows_store::LocalStore>()
        .monitor_history((&key.0, &key.1), (now.saturating_sub(3_600_000), now))
        .map_err(|_| McpError::Database)?;
    let binding = format!("{}:{}:history", key.0, key.1);
    Ok(history_points(points, &binding))
}
fn history_points(
    mut points: Vec<openrows_store::MonitorAggregate>,
    binding: &str,
) -> Vec<openrows_ai::sharing::Evidence> {
    points.sort_unstable_by(|left, right| {
        right
            .collected_ms
            .cmp(&left.collected_ms)
            .then_with(|| left.metric.cmp(&right.metric))
    });
    points
        .into_iter()
        .take(200)
        .map(|point| {
            let mut evidence = investigation::sample(
                binding,
                &point.metric,
                milliseconds(point.collected_ms),
                point.value,
            );
            evidence.scope = if point.metric.starts_with("Database:") {
                "database"
            } else if point.metric.starts_with("sessions:")
                || point.metric.starts_with("Replication:")
            {
                "server"
            } else {
                "unknown"
            }
            .into();
            apply_state(&mut evidence, &point.state);
            evidence
        })
        .collect()
}

fn apply_state(point: &mut openrows_ai::sharing::Evidence, validity: &str) {
    point.state = match validity {
        "ok" if point.value.is_some() => EvidenceState::Ok,
        "gap" => EvidenceState::Gap,
        "counterReset" => EvidenceState::CounterReset,
        "permissionDenied" => EvidenceState::PermissionDenied,
        "unavailable" => EvidenceState::Unavailable,
        _ => EvidenceState::Unknown,
    };
    if point.state != EvidenceState::Ok {
        point.value = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_selects_latest_observations_across_metrics_and_preserves_gaps() {
        let mut points: Vec<_> = (0..200)
            .map(|time| openrows_store::MonitorAggregate {
                metric: "sessions:active".into(),
                collected_ms: time,
                value: Some(1.0),
                state: "ok".into(),
            })
            .collect();
        points.insert(
            0,
            openrows_store::MonitorAggregate {
                metric: "Database:commits".into(),
                collected_ms: 1000,
                value: Some(1.0),
                state: "gap".into(),
            },
        );
        let evidence = history_points(points, "fixture");
        assert_eq!(evidence.len(), 200);
        assert_eq!(evidence[0].collected_ms.to_bits(), 1000.0_f64.to_bits());
        assert_eq!(evidence[0].scope, "database");
        assert_eq!(evidence[0].state, EvidenceState::Gap);
        assert_eq!(evidence[0].value, None);
        assert_eq!(evidence[199].collected_ms.to_bits(), 1.0_f64.to_bits());
    }
}
