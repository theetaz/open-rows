// SPDX-License-Identifier: Apache-2.0
//! Safe source, scope and plan projection without raw activity or probe text.
use openrows_ai::{investigation, sharing::EvidenceState};
type Result = std::result::Result<(), Box<dyn std::error::Error>>;
#[test]
fn projections_drop_activity_identity_query_literals_and_raw_plan_text() -> Result {
    use openrows_monitor::advisor::*;
    let input = AdvisorInput {
        activity_coverage: Coverage {
            collected_ms: 1000.0,
            complete: true,
        },
        locks: vec![LockEvidence {
            identity: "private identity".into(),
            pid: "1234".into(),
            blockers: "5678".into(),
            wait: "private wait address".into(),
            transaction_ms: Some(8.0),
            samples: 2,
        }],
        plans: vec![PlanEvidence {
            statement_id: "private query".into(),
            collected_ms: 1000.0,
            nodes: vec![NodeEvidence {
                operator: "private vendor predicate".into(),
                estimated_rows: Some(2.0),
                ..NodeEvidence::default()
            }],
            ..PlanEvidence::default()
        }],
        ..AdvisorInput::default()
    };
    let values = serde_json::to_string(&(
        investigation::locks("connection", &input),
        investigation::plans("connection", &input),
        investigation::plan_context(&input),
    ))?;
    for secret in ["private", "1234", "5678"] {
        assert!(!values.contains(secret));
    }
    assert!(values.contains("lock:edge"));
    Ok(())
}

#[test]
fn health_preserves_native_scope_and_refusals_without_exposing_probe_details() -> Result {
    use openrows_monitor::{Descriptor, SampleState, Scope, Source, SourceStatus};
    let statuses = vec![SourceStatus {
        source: Source::Activity,
        state: SampleState::PermissionDenied("private address and user".into()),
        collected_ms: 0.0,
        last_success_ms: None,
        interval_ms: 5000,
        duration_ms: 0.0,
        sequence: 0,
        partial: false,
    }];
    let descriptors = vec![Descriptor {
        source: Source::Activity,
        state: SampleState::Unknown,
        scope: Scope::Server,
        interval_ms: 5000,
        guidance: Some("private configuration".into()),
        coverage: "private metadata".into(),
    }];
    let points = investigation::health("fixture", &statuses, &descriptors, 2000.0);
    assert_eq!(points[0].scope, "server");
    assert_eq!(points[0].collected_ms, 2000.0);
    assert_eq!(points[0].value, None);
    assert_eq!(points[0].state, EvidenceState::PermissionDenied);
    assert!(!serde_json::to_string(&points)?.contains("private"));
    let point = investigation::observation(
        "fixture",
        &openrows_monitor::alerts::Observation {
            metric: "sessions:active".into(),
            scope: Scope::Server,
            wall_ms: 2000.0,
            value: Some(8.0),
            state: SampleState::Gap,
        },
    );
    assert_eq!(point.scope, "server");
    assert_eq!(point.value, None);
    assert_eq!(point.state, EvidenceState::Gap);
    Ok(())
}
