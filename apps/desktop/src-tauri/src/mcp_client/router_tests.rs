// SPDX-License-Identifier: Apache-2.0
use super::*;
#[test]
fn transport_arguments_are_exactly_the_gate_filtered_value() {
    let args = serde_json::json!({"query":"SELECT ?"});
    let preview = serde_json::json!({"samples":[{"rows":[[args.to_string()]]}]}).to_string();
    assert_eq!(gated_arguments(&preview).unwrap(), args);
    let redacted = serde_json::json!({"samples":[{"rows":[["‹redacted›"]]}]}).to_string();
    assert!(matches!(
        gated_arguments(&redacted),
        Err(AgentError::Refused)
    ));
    assert!(matches!(gated_arguments("{}"), Err(AgentError::Refused)));
}
#[test]
fn sample_column_redaction_refuses_external_arguments_before_transport()
-> Result<(), Box<dyn std::error::Error>> {
    use openrows_ai::{
        AiControl, ProviderConfig, ProviderKind,
        sharing::{GateRequest, RedactionRule, SharingLevel, SharingPolicy, SharingStore},
    };
    let store = SharingStore::memory(10)?;
    store.save(
        "connection",
        SharingPolicy {
            level: SharingLevel::SampleRows,
            providers: vec!["fixture".into()],
            rules: vec![RedactionRule::Glob("external.*".into())],
            ..SharingPolicy::default()
        },
        false,
    )?;
    let control = AiControl::default();
    control.set_enabled(true);
    let provider = ProviderConfig {
        id: "fixture".into(),
        name: "Fixture".into(),
        kind: ProviderKind::OpenAi,
        base_url: "https://api.openai.com/v1/".into(),
        allow_insecure_remote: false,
        secret_ref: String::new(),
    };
    let gate = store.gate(GateRequest {
        connection: "connection",
        production: false,
        provider: &provider,
        control: &control,
        dialect: openrows_sql::Dialect::Postgres,
        bundle: bundle(&serde_json::json!({"q":"private value"}))?,
    })?;
    let preview = gate.preview();
    assert!(!preview.payload.contains("private value"));
    assert!(matches!(
        gated_arguments(&preview.payload),
        Err(AgentError::Refused)
    ));
    Ok(())
}
