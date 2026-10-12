// SPDX-License-Identifier: Apache-2.0
//! Finite budgets, explicit tool/provider selection and scrubbed audit retention.
use openrows_ai::{
    scheduled::InvestigationSchedule,
    sharing::{InvestigationAudit, SharingStore},
};
type Result = std::result::Result<(), Box<dyn std::error::Error>>;
fn config() -> InvestigationSchedule {
    InvestigationSchedule {
        enabled: false,
        provider: "fixture".into(),
        model: "fixture-model".into(),
        interval_secs: 300,
        max_runs: 2,
        steps: 8,
        tokens: 65536,
        timeout_secs: 30,
        max_cost_micros: 1000,
        price_per_million_micros: 10000000,
        tools: vec!["read_health_snapshot".into()],
    }
}
#[test]
fn cost_bounds_reduce_tokens_before_any_provider_call() -> Result {
    let job = config();
    let limits = job.limits()?;
    assert_eq!(limits.tokens, 100);
    assert_eq!(job.cost_micros(limits.tokens), 1000);
    let mut invalid = config();
    invalid.max_cost_micros = 1;
    invalid.price_per_million_micros = u32::MAX;
    assert!(invalid.limits().is_err());
    Ok(())
}
#[test]
fn invalid_limits_unapproved_tools_and_unknown_prices_fail_closed() {
    let mutations: [fn(&mut InvestigationSchedule); 12] = [
        |job: &mut InvestigationSchedule| job.steps = 0,
        |job: &mut InvestigationSchedule| job.steps = 13,
        |job: &mut InvestigationSchedule| job.tokens = 65537,
        |job: &mut InvestigationSchedule| job.timeout_secs = 61,
        |job: &mut InvestigationSchedule| job.interval_secs = 299,
        |job: &mut InvestigationSchedule| job.max_runs = 0,
        |job: &mut InvestigationSchedule| job.max_runs = 1001,
        |job: &mut InvestigationSchedule| job.price_per_million_micros = 0,
        |job: &mut InvestigationSchedule| job.provider.clear(),
        |job: &mut InvestigationSchedule| job.model.clear(),
        |job: &mut InvestigationSchedule| job.tools = vec!["run_read_query".into()],
        |job: &mut InvestigationSchedule| job.tools.clear(),
    ];
    for mutate in mutations {
        let mut job = config();
        mutate(&mut job);
        assert!(job.limits().is_err());
    }
    assert!(!config().enabled);
}
#[test]
fn audit_retention_is_scrubbed_bounded_and_purgeable() -> Result {
    let store = SharingStore::memory(2)?;
    let mut entry = InvestigationAudit {
        time: "1000".into(),
        connection: "connection".into(),
        provider: "fixture".into(),
        model: "fixture-model".into(),
        tools: vec!["read_health_snapshot".into()],
        evidence: vec!["a".repeat(64)],
        tokens: 100,
        cost_micros: 1000,
        outcome: "completed".into(),
    };
    for time in [1000, 2000, 3000] {
        entry.time = time.to_string();
        store.record_investigation(&entry)?;
    }
    let entries = store.investigations()?;
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].time, "3000");
    let json = serde_json::to_value(&entries[0])?;
    for field in ["question", "payload", "sql", "answer", "key", "arguments"] {
        assert!(json.get(field).is_none());
    }
    entry.tools = vec!["DROP TABLE secret".into()];
    assert!(store.record_investigation(&entry).is_err());
    entry.tools = vec![];
    entry.evidence = vec!["forged".into()];
    assert!(store.record_investigation(&entry).is_err());
    store.purge()?;
    assert!(store.investigations()?.is_empty());
    Ok(())
}
