// SPDX-License-Identifier: Apache-2.0
//! Project existing bounded collectors; no additional database query or activity text.
use crate::sharing::{Evidence, EvidenceState};
use openrows_monitor::{Descriptor, SampleState, Scope, SourceStatus, advisor::AdvisorInput};
use sha2::{Digest, Sha256};

/// Hash the native connection/generation/sample binding before disclosure.
#[must_use]
pub fn sample(binding: &str, metric: &str, time: f64, value: Option<f64>) -> Evidence {
    Evidence {
        id: format!(
            "{:x}",
            Sha256::digest(format!("{binding}:{metric}:{time}").as_bytes())
        ),
        metric: metric.into(),
        collected_ms: time,
        value,
        state: if value.is_some() {
            EvidenceState::Ok
        } else {
            EvidenceState::Unknown
        },
        scope: "database".into(),
    }
}
fn state(value: &SampleState) -> EvidenceState {
    match value {
        SampleState::Ok => EvidenceState::Ok,
        SampleState::Unknown => EvidenceState::Unknown,
        SampleState::Gap => EvidenceState::Gap,
        SampleState::CounterReset => EvidenceState::CounterReset,
        SampleState::Unavailable(_) => EvidenceState::Unavailable,
        SampleState::PermissionDenied(_) => EvidenceState::PermissionDenied,
    }
}
/// Source freshness/visibility stays explicit, without vendor messages.
#[must_use]
pub fn health(
    binding: &str,
    sources: &[SourceStatus],
    descriptors: &[Descriptor],
    now: f64,
) -> Vec<Evidence> {
    sources
        .iter()
        .map(|source| {
            let mut point = sample(
                binding,
                &format!("{:?}:status", source.source),
                if source.collected_ms > 0.0 {
                    source.collected_ms
                } else {
                    now
                },
                None,
            );
            point.scope = descriptors
                .iter()
                .find(|descriptor| descriptor.source == source.source)
                .map_or("unknown", |descriptor| match descriptor.scope {
                    Scope::Database => "database",
                    Scope::Server => "server",
                })
                .into();
            point.state = if source.partial {
                EvidenceState::Partial
            } else {
                state(&source.state)
            };
            point
        })
        .collect()
}
/// Read reset-aware query intervals. Query identities are hashed; text is excluded.
#[must_use]
pub fn queries(binding: &str, input: &AdvisorInput) -> Vec<Evidence> {
    input
        .intervals
        .iter()
        .rev()
        .take(50)
        .flat_map(|interval| {
            let key = format!("{binding}:{}", interval.statement_id);
            [
                ("query:calls", interval.calls),
                ("query:total_ms", interval.total_ms),
                ("query:rows", interval.rows),
            ]
            .into_iter()
            .map(move |(metric, value)| {
                let valid = interval.state == SampleState::Ok;
                let mut point =
                    sample(&key, metric, interval.collected_ms, value.filter(|_| valid));
                point.scope = format!(
                    "query:{:x}",
                    Sha256::digest(interval.statement_id.as_bytes())
                );
                if !valid {
                    point.state = state(&interval.state);
                }
                point
            })
        })
        .collect()
}
/// The graph uses pseudonymous session nodes; users/addresses/wait strings are omitted.
#[must_use]
pub fn locks(binding: &str, input: &AdvisorInput) -> Vec<Evidence> {
    input
        .locks
        .iter()
        .take(20)
        .flat_map(|lock| {
            let key = format!("{binding}:{}", lock.identity);
            let scope = format!("session:{:x}", Sha256::digest(lock.pid.as_bytes()));
            let mut age = sample(
                &key,
                "lock:transaction_ms",
                input.activity_coverage.collected_ms,
                lock.transaction_ms,
            );
            age.scope = scope.clone();
            let mut points = vec![age];
            for blocker in lock
                .blockers
                .split(',')
                .take(8)
                .filter(|pid| !pid.trim().is_empty())
            {
                let edge = format!("{key}:{}", blocker.trim());
                let mut point = sample(
                    &edge,
                    "lock:edge",
                    input.activity_coverage.collected_ms,
                    Some(1.0),
                );
                point.scope = format!(
                    "{}>{:x}",
                    &scope[8..32],
                    Sha256::digest(blocker.trim().as_bytes())
                );
                points.push(point);
            }
            if !input.activity_coverage.complete {
                for point in &mut points {
                    point.value = None;
                    point.state = EvidenceState::Partial;
                }
            }
            points
        })
        .take(200)
        .collect()
}
/// Existing plan node estimates are labelled as estimates; no ANALYZE is issued.
#[must_use]
pub fn plans(binding: &str, input: &AdvisorInput) -> Vec<Evidence> {
    input
        .plans
        .iter()
        .take(10)
        .flat_map(|plan| {
            plan.nodes.iter().take(20).map(move |node| {
                let key = format!("{binding}:{}:{}", plan.statement_id, node.id);
                let mut point = sample(
                    &key,
                    "plan:estimated_rows",
                    plan.collected_ms,
                    node.estimated_rows,
                );
                point.scope = format!("plan:{:x}", Sha256::digest(key.as_bytes()));
                point
            })
        })
        .collect()
}

/// Whitelisted existing plan operators and catalog relations; predicates are excluded.
#[must_use]
pub fn plan_context(input: &AdvisorInput) -> Vec<crate::sharing::Plan> {
    input
        .plans
        .iter()
        .take(10)
        .flat_map(|plan| {
            plan.nodes.iter().take(20).filter_map(|node| {
                let estimated_rows = node
                    .estimated_rows?
                    .floor()
                    .to_string()
                    .parse::<u32>()
                    .ok()?;
                let relation = input
                    .tables
                    .iter()
                    .find(|table| Some(&table.id) == node.table_id.as_ref())
                    .map_or_else(String::new, |table| {
                        format!("{}.{}", table.schema, table.name)
                    });
                Some(crate::sharing::Plan {
                    operation: operation(&node.operator),
                    parent: None,
                    relation,
                    predicate: String::new(),
                    estimated_rows,
                })
            })
        })
        .collect()
}
fn operation(operator: &str) -> crate::sharing::PlanOperation {
    use crate::sharing::PlanOperation;
    match operator {
        "Seq Scan" | "Index Scan" | "Index Only Scan" | "Bitmap Heap Scan"
        | "Bitmap Index Scan" => PlanOperation::Scan,
        "Nested Loop" | "Hash Join" | "Merge Join" => PlanOperation::Join,
        "Sort" | "Incremental Sort" => PlanOperation::Sort,
        "Aggregate" | "GroupAggregate" | "HashAggregate" => PlanOperation::Aggregate,
        _ => PlanOperation::Other,
    }
}

/// A scalar observation retains native server/database scope and its missing state.
#[must_use]
pub fn observation(binding: &str, value: &openrows_monitor::alerts::Observation) -> Evidence {
    let mut point = sample(binding, &value.metric, value.wall_ms, value.value);
    point.scope = match value.scope {
        Scope::Database => "database",
        Scope::Server => "server",
    }
    .into();
    if value.state != SampleState::Ok {
        point.state = state(&value.state);
        point.value = None;
    }
    point
}
