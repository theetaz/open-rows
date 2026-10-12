// SPDX-License-Identifier: Apache-2.0
//! Monitoring disclosure, grounded artifacts and scheduled accounting safety.
mod support;
use openrows_ai::{
    investigation::{self, ArtifactProposal, Ledger},
    sharing::*,
};
use openrows_sql::Dialect;
use support::sharing::{self, Result};
fn bundle() -> ContextBundle {
    ContextBundle {
        evidence: vec![investigation::sample(
            "connection:generation:sample",
            "sessions:active",
            1000.0,
            Some(7.0),
        )],
        ..ContextBundle::default()
    }
}
#[test]
fn telemetry_matrix_denies_none_schema_provider_production_and_missing_consent() -> Result {
    let store = SharingStore::memory(10)?;
    let control = openrows_ai::AiControl::default();
    control.set_enabled(true);
    let provider = sharing::provider();
    for level in [
        SharingLevel::None,
        SharingLevel::SchemaOnly,
        SharingLevel::SchemaAndStats,
        SharingLevel::SampleRows,
    ] {
        for telemetry in [false, true] {
            let mut policy = sharing::policy(level);
            policy.telemetry = telemetry;
            store.save("connection", policy, false)?;
            let result = store.gate(GateRequest {
                connection: "connection",
                production: false,
                provider: &provider,
                control: &control,
                dialect: Dialect::Postgres,
                bundle: bundle(),
            });
            assert_eq!(
                result.is_ok(),
                telemetry && level >= SharingLevel::SchemaAndStats
            );
        }
    }
    let mut policy = sharing::policy(SharingLevel::SchemaAndStats);
    policy.providers.clear();
    store.save("connection", policy, false)?;
    assert!(
        store
            .gate(sharing::request(&provider, &control, bundle()))
            .is_err()
    );
    store.save(
        "connection",
        sharing::policy(SharingLevel::SchemaAndStats),
        false,
    )?;
    let mut request = sharing::request(&provider, &control, bundle());
    request.production = true;
    assert!(matches!(store.gate(request), Err(Refusal::Production)));
    Ok(())
}
#[test]
fn redaction_and_truncation_define_the_actual_citation_universe() -> Result {
    let store = SharingStore::memory(10)?;
    let control = openrows_ai::AiControl::default();
    control.set_enabled(true);
    let provider = sharing::provider();
    let mut policy = sharing::policy(SharingLevel::SchemaAndStats);
    policy.rules.push(RedactionRule::Glob("sessions:*".into()));
    store.save("connection", policy, false)?;
    let gated = store.gate(sharing::request(&provider, &control, bundle()))?;
    let sent: ContextBundle = serde_json::from_str(&gated.preview().payload)?;
    assert_eq!(sent.evidence[0].metric, "‹redacted›");
    assert_eq!(sent.evidence[0].value, None);
    let ledger = Ledger::default();
    ledger.record("connection", &gated)?;
    let id = sent.evidence[0].id.clone();
    let proposal = serde_json::from_value(
        serde_json::json!({"kind":"report","title":"Health","findings":[],"evidence":[id],"chart":true}),
    )?;
    let artifact = ledger.propose("connection", Dialect::Postgres, proposal)?;
    assert!(!artifact.document().contains("sessions:active"));
    assert!(
        ledger
            .propose("other", Dialect::Postgres, report(&id)?)
            .is_err()
    );
    assert!(
        ledger
            .propose("connection", Dialect::Postgres, report(&"a".repeat(64))?)
            .is_err()
    );
    let mut input = bundle();
    input.evidence.extend(
        (0..20)
            .map(|i| investigation::sample(&i.to_string(), "sessions:active", 1000.0, Some(3.0))),
    );
    let removed = input.evidence.last().ok_or("missing")?.id.clone();
    let truncated = store.gate_with_limits(
        sharing::request(&provider, &control, input),
        Limits {
            bytes: 512,
            tokens: 512,
            ..Limits::default()
        },
    )?;
    let ledger = Ledger::default();
    ledger.record("connection", &truncated)?;
    assert!(truncated.preview().truncated);
    assert!(
        ledger
            .propose("connection", Dialect::Postgres, report(&removed)?)
            .is_err()
    );
    Ok(())
}
fn report(id: &str) -> Result<ArtifactProposal> {
    Ok(serde_json::from_value(
        serde_json::json!({"kind":"report","title":"Report","findings":[],"evidence":[id],"chart":false}),
    )?)
}
#[test]
fn invalid_values_and_payload_fields_are_rejected() -> Result {
    let store = SharingStore::memory(10)?;
    let control = openrows_ai::AiControl::default();
    control.set_enabled(true);
    let provider = sharing::provider();
    store.save(
        "connection",
        sharing::policy(SharingLevel::SchemaAndStats),
        false,
    )?;
    for value in [f64::NAN, f64::INFINITY] {
        let mut input = bundle();
        input.evidence[0].value = Some(value);
        assert!(
            store
                .gate(sharing::request(&provider, &control, input))
                .is_err()
        );
    }
    let mut input = bundle();
    input.evidence[0].state = EvidenceState::Gap;
    assert!(
        store
            .gate(sharing::request(&provider, &control, input))
            .is_err()
    );
    for scope in ["raw-user", "192.168.1.1", "query:unhashed-private-sql"] {
        let mut input = bundle();
        input.evidence[0].scope = scope.into();
        assert!(
            store
                .gate(sharing::request(&provider, &control, input))
                .is_err()
        );
    }
    assert!(serde_json::from_value::<ArtifactProposal>(serde_json::json!({"kind":"report","title":"Fake","evidence":[],"findings":[],"chart":true,"measurements":[123]})).is_err());
    Ok(())
}
#[test]
fn reports_keep_metrics_separate_preserve_gaps_and_include_every_cited_sample() -> Result {
    let store = SharingStore::memory(10)?;
    let control = openrows_ai::AiControl::default();
    control.set_enabled(true);
    let provider = sharing::provider();
    store.save(
        "connection",
        sharing::policy(SharingLevel::SchemaAndStats),
        false,
    )?;
    let points = vec![
        investigation::sample("first", "sessions:active", 2000.0, Some(4.0)),
        investigation::sample("second", "sessions:active", 1000.0, None),
        investigation::sample("third", "query:total_ms", 2000.0, Some(12.0)),
    ];
    let ids: Vec<_> = points.iter().map(|point| point.id.clone()).collect();
    let payload = store.gate(sharing::request(
        &provider,
        &control,
        ContextBundle {
            evidence: points,
            ..ContextBundle::default()
        },
    ))?;
    let ledger = Ledger::default();
    ledger.record("connection", &payload)?;
    let proposal = serde_json::from_value(
        serde_json::json!({"kind":"report","title":"Health","findings":[],"evidence":ids,"chart":true}),
    )?;
    let artifact = ledger.propose("connection", Dialect::Postgres, proposal)?;
    let report: serde_json::Value = serde_json::from_str(artifact.document())?;
    let sections = report["sections"].as_array().ok_or("sections missing")?;
    let charts: Vec<_> = sections
        .iter()
        .filter(|section| section["kind"] == "chart")
        .collect();
    assert_eq!(charts.len(), 2);
    let sessions = charts
        .iter()
        .find(|section| section["title"] == "sessions:active")
        .ok_or("sessions chart missing")?;
    assert_eq!(sessions["series"][0]["points"][0]["x"], 1000.0);
    assert_eq!(
        sessions["series"][0]["points"][0]["y"],
        serde_json::Value::Null
    );
    let proposal = serde_json::from_value(
        serde_json::json!({"kind":"report","title":"Incomplete","evidence":[ids[0]],"findings":[{"interpretation":"A finding","uncertainty":"Limited coverage","evidence":[ids[2]]}],"chart":false}),
    )?;
    assert!(
        ledger
            .propose("connection", Dialect::Postgres, proposal)
            .is_err()
    );
    Ok(())
}
