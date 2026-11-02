// SPDX-License-Identifier: Apache-2.0
//! Health and report grounding evaluation (docs/28 §8 item 10, M45-09). A
//! fixed set of recorded model proposals is replayed against evidence that was
//! actually delivered through the sharing gate. Grounded proposals must be
//! accepted; fabricated, foreign, uncited or model-measured ones must be
//! refused with the documented typed reason.
mod support;
use openrows_ai::{
    investigation::{self, ArtifactProposal, Ledger},
    sharing::*,
};
use openrows_sql::Dialect;
use serde::Deserialize;
use support::sharing::{self, Result};

#[derive(Deserialize)]
struct EvalSet {
    evidence: Vec<Point>,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Point {
    binding: String,
    metric: String,
    time: f64,
    value: f64,
}
#[derive(Deserialize)]
struct Case {
    id: String,
    #[serde(default)]
    connection: Option<String>,
    proposal: serde_json::Value,
    expect: String,
}

/// Gate the fixture evidence and record exactly what was delivered.
fn delivered(set: &EvalSet) -> Result<(Ledger, Vec<String>)> {
    let store = SharingStore::memory(10)?;
    let control = openrows_ai::AiControl::default();
    control.set_enabled(true);
    let provider = sharing::provider();
    store.save(
        "connection",
        sharing::policy(SharingLevel::SchemaAndStats),
        false,
    )?;
    let evidence = set
        .evidence
        .iter()
        .map(|point| {
            investigation::sample(&point.binding, &point.metric, point.time, Some(point.value))
        })
        .collect();
    let payload = store.gate(sharing::request(
        &provider,
        &control,
        ContextBundle {
            evidence,
            ..ContextBundle::default()
        },
    ))?;
    let delivered_bundle: ContextBundle = serde_json::from_str(&payload.preview().payload)?;
    let ledger = Ledger::default();
    ledger.record("connection", &payload)?;
    Ok((
        ledger,
        delivered_bundle
            .evidence
            .into_iter()
            .map(|point| point.id)
            .collect(),
    ))
}

/// Replace `@n` placeholders with the n-th delivered evidence id.
fn bind(value: &serde_json::Value, ids: &[String]) -> serde_json::Value {
    match value {
        serde_json::Value::String(text) => text
            .strip_prefix('@')
            .and_then(|index| index.parse::<usize>().ok())
            .and_then(|index| ids.get(index))
            .map_or_else(|| value.clone(), |id| serde_json::Value::String(id.clone())),
        serde_json::Value::Array(items) => items.iter().map(|item| bind(item, ids)).collect(),
        serde_json::Value::Object(map) => map
            .iter()
            .map(|(key, item)| (key.clone(), bind(item, ids)))
            .collect::<serde_json::Map<_, _>>()
            .into(),
        _ => value.clone(),
    }
}

/// `accepted`, or the camelCase rejection the ledger returned.
fn outcome(ledger: &Ledger, case: &Case, ids: &[String]) -> Result<String> {
    let Ok(proposal) = serde_json::from_value::<ArtifactProposal>(bind(&case.proposal, ids)) else {
        return Ok("invalidDocument".into());
    };
    let connection = case.connection.as_deref().unwrap_or("connection");
    Ok(
        match ledger.propose(connection, Dialect::Postgres, proposal) {
            Ok(_) => "accepted".into(),
            Err(rejection) => serde_json::to_value(rejection)?
                .as_str()
                .ok_or("rejection is not a string")?
                .to_owned(),
        },
    )
}

#[test]
fn recorded_proposals_are_accepted_only_when_grounded_in_delivered_evidence() -> Result {
    let set: EvalSet = serde_json::from_str(include_str!("eval/grounding.json"))?;
    let (ledger, ids) = delivered(&set)?;
    assert_eq!(
        ids.len(),
        set.evidence.len(),
        "every fixture sample is delivered"
    );
    let mut grounded = 0;
    for case in &set.cases {
        let result = outcome(&ledger, case, &ids)?;
        assert_eq!(result, case.expect, "{}", case.id);
        grounded += usize::from(result == "accepted");
    }
    assert_eq!(grounded, 3, "three grounded proposals, six refusals");
    Ok(())
}
