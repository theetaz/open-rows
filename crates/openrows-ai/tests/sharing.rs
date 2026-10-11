// SPDX-License-Identifier: Apache-2.0
//! Golden context, deterministic bounds and default-deny permissions.
mod support;
use openrows_ai::{AiControl, ProviderKind, sharing::*};
use support::sharing::*;
#[test]
fn golden_payload_at_every_level_and_caps() -> Result {
    let store = SharingStore::memory(1000)?;
    let control = AiControl::default();
    control.set_enabled(true);
    let config = provider();
    let levels = [
        SharingLevel::None,
        SharingLevel::SchemaOnly,
        SharingLevel::SchemaAndStats,
        SharingLevel::SampleRows,
    ];
    let goldens = [
        include_str!("golden/none.json"),
        include_str!("golden/schema.json"),
        include_str!("golden/stats.json"),
        include_str!("golden/samples.json"),
    ];
    for (level, golden) in levels.into_iter().zip(goldens) {
        store.save("connection", policy(level), false)?;
        let gated = store.gate_with_limits(
            request(&config, &control, fixture(level)),
            Limits {
                rows: 2,
                cells: 6,
                ..Limits::default()
            },
        );
        if level == SharingLevel::None {
            assert_eq!(gated.err(), Some(Refusal::LevelTooLow));
            assert_eq!(golden.trim(), "\"levelTooLow\"");
            continue;
        }
        let preview = gated?.preview();
        assert_eq!(preview.payload, golden.trim());
        assert_eq!(preview.bytes as usize, preview.payload.len());
        assert_eq!(preview.tokens, preview.bytes);
        assert!(!preview.payload.contains("private"));
        assert!(!preview.payload.contains("hidden"));
        assert_eq!(preview.truncated, level == SharingLevel::SampleRows);
    }
    Ok(())
}
#[test]
fn default_none_and_each_refusal() -> Result {
    let store = SharingStore::memory(1000)?;
    let control = AiControl::default();
    let mut config = provider();
    assert_eq!(
        store
            .gate(request(&config, &control, ContextBundle::default()))
            .err(),
        Some(Refusal::AiOff)
    );
    control.set_enabled(true);
    assert_eq!(
        store
            .gate(request(&config, &control, ContextBundle::default()))
            .err(),
        Some(Refusal::LevelTooLow)
    );
    store.save("connection", policy(SharingLevel::SchemaOnly), false)?;
    let mut production = request(&config, &control, ContextBundle::default());
    production.production = true;
    assert_eq!(store.gate(production).err(), Some(Refusal::Production));
    assert_eq!(
        store.save("connection", policy(SharingLevel::SchemaOnly), true),
        Err(Refusal::Production)
    );
    config.id = "other".into();
    assert_eq!(
        store
            .gate(request(&config, &control, ContextBundle::default()))
            .err(),
        Some(Refusal::ProviderNotAllowed)
    );
    Ok(())
}
#[test]
fn local_endpoint_and_telemetry_refusals() -> Result {
    let store = SharingStore::memory(1000)?;
    let control = AiControl::default();
    control.set_enabled(true);
    let mut config = provider();
    let mut local = policy(SharingLevel::SampleRows);
    local.local_only = true;
    store.save("connection", local, false)?;
    assert_eq!(
        store
            .gate(request(&config, &control, ContextBundle::default()))
            .err(),
        Some(Refusal::LocalOnly)
    );
    config.kind = ProviderKind::Ollama;
    config.allow_insecure_remote = true;
    assert_eq!(
        store
            .gate(request(&config, &control, ContextBundle::default()))
            .err(),
        Some(Refusal::LocalOnly)
    );
    config.base_url = "http://127.0.0.1:11434/".into();
    assert!(
        store
            .gate(request(&config, &control, ContextBundle::default()))
            .is_ok()
    );
    store.save(
        "connection",
        SharingPolicy {
            telemetry: false,
            ..policy(SharingLevel::SchemaAndStats)
        },
        false,
    )?;
    assert_eq!(
        store
            .gate(request(
                &config,
                &control,
                fixture(SharingLevel::SchemaAndStats)
            ))
            .err(),
        Some(Refusal::Telemetry)
    );
    assert_eq!(
        store
            .gate(request(
                &config,
                &control,
                fixture(SharingLevel::SampleRows)
            ))
            .err(),
        Some(Refusal::LevelTooLow)
    );
    Ok(())
}
#[test]
fn production_opt_in_does_not_carry_into_retag_or_duplicate() -> Result {
    let store = SharingStore::memory(1000)?;
    let mut approved = policy(SharingLevel::SchemaOnly);
    approved.production_confirmed = true;
    store.save("connection", approved.clone(), true)?;
    assert!(store.policy("connection")?.production_confirmed);
    store.save("connection", approved, false)?;
    assert!(!store.policy("connection")?.production_confirmed);
    assert_eq!(store.policy("duplicate")?.level, SharingLevel::None);
    Ok(())
}
#[test]
fn rules_are_case_insensitive_and_match_qualified_quoted_identifiers() -> Result {
    let store = SharingStore::memory(1000)?;
    let mut policy = policy(SharingLevel::SampleRows);
    policy
        .rules
        .push(RedactionRule::Regex(r"(?i)public\.people\.id".into()));
    store.save("connection", policy, false)?;
    let control = AiControl::default();
    control.set_enabled(true);
    let config = provider();
    let bundle = ContextBundle {
        query: Some(
            "SELECT \"PUBLIC\" . \"PEOPLE\" . \"ID\", phone, card FROM public.people".into(),
        ),
        ..fixture(SharingLevel::SampleRows)
    };
    let preview = store.gate(request(&config, &control, bundle))?.preview();
    assert!(!preview.payload.contains("ID") && !preview.payload.contains("private-row"));
    assert!(!preview.payload.contains("phone") && !preview.payload.contains("card"));
    assert!(preview.payload.contains("‹redacted›"));
    assert!(
        store
            .save(
                "connection",
                SharingPolicy {
                    rules: vec![RedactionRule::Regex("[".into())],
                    ..SharingPolicy::default()
                },
                false
            )
            .is_err()
    );
    Ok(())
}
#[test]
fn cell_utf8_byte_and_token_caps_are_deterministic_and_apply_to_json_envelope() -> Result {
    let store = SharingStore::memory(1000)?;
    store.save("connection", policy(SharingLevel::SampleRows), false)?;
    let control = AiControl::default();
    control.set_enabled(true);
    let config = provider();
    let bundle = ContextBundle {
        samples: vec![Sample {
            table: "t".into(),
            columns: vec!["value".into()],
            rows: vec![vec![Some("é\\\"".repeat(300))]],
        }],
        ..ContextBundle::default()
    };
    let limit = Limits {
        field_bytes: 15,
        bytes: 180,
        tokens: 160,
        ..Limits::default()
    };
    let first = store
        .gate_with_limits(request(&config, &control, bundle.clone()), limit)?
        .preview();
    let second = store
        .gate_with_limits(request(&config, &control, bundle), limit)?
        .preview();
    assert_eq!(first.payload, second.payload);
    assert!(first.bytes <= 160);
    assert!(first.truncated);
    assert!(serde_json::from_str::<serde_json::Value>(&first.payload).is_ok());
    assert!(
        store
            .gate_with_limits(
                request(&config, &control, ContextBundle::default()),
                Limits {
                    rows: 21,
                    ..Limits::default()
                }
            )
            .is_err()
    );
    Ok(())
}
