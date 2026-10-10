// SPDX-License-Identifier: Apache-2.0
//! Open export schema, secret exclusion, round trip and preview-only behavior.
use openrows_secrets::{MemorySecretStore, SecretString};
use openrows_store::profile::{
    ConnectionProfile, EnvKind, EnvTag, PasswordChange, ProfileFiles, ProfileService,
    SecretChanges, ServerParams, SshAuthMethod, SshSettings,
    exchange::{self, Conflict, ImportChoice, ImportDecision},
};
use std::sync::Arc;
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn profiles() -> Vec<ConnectionProfile> {
    (0..3)
        .map(|index| {
            let mut profile = ConnectionProfile::new_postgres(
                &format!("profile {index}"),
                ServerParams::default(),
                EnvTag::new(EnvKind::Prod),
            );
            profile.group = Some("group".into());
            profile.tags = vec!["tag".into()];
            profile.safety.read_only = true;
            profile.redact_history = index == 0;
            profile.tls.ca_file = Some("/certs/ca.pem".into());
            profile.tls.client_key = Some("/certs/key.pem".into());
            profile.ssh = Some(SshSettings {
                host: "bastion".into(),
                port: None,
                user: None,
                auth: SshAuthMethod::PrivateKey {
                    key_file: "/keys/id".into(),
                },
                jump_hosts: vec!["hop".into()],
            });
            profile
        })
        .collect()
}
#[test]
fn three_profiles_validate_schema_without_any_saved_secret_and_round_trip() -> TestResult {
    let dir = tempfile::tempdir()?;
    let service = ProfileService::new(
        ProfileFiles::new(dir.path()),
        Arc::new(MemorySecretStore::new()),
    );
    let changes = SecretChanges {
        password: PasswordChange::Set(SecretString::from("test-only-password")),
        ssh_passphrase: PasswordChange::Set(SecretString::from("test-only-ssh")),
        tls_key_passphrase: PasswordChange::Set(SecretString::from("test-only-tls")),
        ..SecretChanges::default()
    };
    let saved = profiles()
        .into_iter()
        .map(|profile| service.create_with(profile, &changes))
        .collect::<Result<Vec<_>, _>>()?;
    let text = exchange::export(&saved)?;
    for secret in ["test-only-password", "test-only-ssh", "test-only-tls"] {
        assert!(!text.contains(secret));
    }
    assert!(!text.contains("secrets"));
    let schema: serde_json::Value = serde_json::from_str(exchange::CONNECTIONS_SCHEMA)?;
    let validator = jsonschema::validator_for(&schema)?;
    assert!(validator.is_valid(&serde_json::from_str(&text)?));
    let empty = tempfile::tempdir()?;
    let imported = ProfileService::new(
        ProfileFiles::new(empty.path()),
        Arc::new(MemorySecretStore::new()),
    );
    let batch = exchange::parse(&text)?;
    assert_eq!(exchange::preview(&batch, &[])?, vec![Conflict::New; 3]);
    assert!(imported.list()?.profiles.is_empty());
    let decisions = vec![
        ImportDecision {
            choice: ImportChoice::KeepBoth,
            save_password: false
        };
        3
    ];
    let created = exchange::apply_import(&imported, batch, &decisions)?;
    for (before, after) in saved.iter().zip(&created) {
        assert!(exchange::equivalent(before, after)?);
        assert_ne!(before.id, after.id);
        assert!(after.imported);
        assert_eq!(before.redact_history, after.redact_history);
    }
    assert_eq!(
        exchange::preview(&exchange::parse(&text)?, &created)?,
        vec![Conflict::Identical; 3]
    );
    Ok(())
}
#[test]
fn future_version_and_malformed_json_have_typed_readable_errors() {
    assert!(matches!(
        exchange::parse(r#"{"format":"openrows.connections","version":2,"profiles":[]}"#),
        Err(exchange::ExchangeError::Version(2))
    ));
    let error = exchange::parse("{\n  broken").unwrap_err();
    assert!(matches!(
        error,
        exchange::ExchangeError::Json { line: 2, column: 3 }
    ));
    assert!(error.to_string().contains("line 2, column 3"));
}
#[test]
fn unknown_nested_fields_warn_without_values_and_conflicts_obey_decisions() -> TestResult {
    let dir = tempfile::tempdir()?;
    let service = ProfileService::new(
        ProfileFiles::new(dir.path()),
        Arc::new(MemorySecretStore::new()),
    );
    let saved = service.create(profiles().remove(0), None)?;
    let mut incoming = saved.clone();
    incoming.params.port = 6432;
    let mut value: serde_json::Value = serde_json::from_str(&exchange::export(&[incoming])?)?;
    value["later"] = serde_json::json!("test-only");
    value["profiles"][0]["tls"]["later"] = serde_json::json!("test-only");
    let text = value.to_string();
    let batch = exchange::parse(&text)?;
    assert_eq!(batch.warnings.len(), 2);
    assert!(!format!("{:?}", batch.warnings).contains("test-only"));
    assert_eq!(
        exchange::preview(&batch, std::slice::from_ref(&saved))?,
        vec![Conflict::SameName(saved.id)]
    );
    let skip = ImportDecision {
        choice: ImportChoice::Skip,
        save_password: false,
    };
    assert!(exchange::apply_import(&service, batch, &[skip])?.is_empty());
    let keep = ImportDecision {
        choice: ImportChoice::KeepBoth,
        ..skip
    };
    let kept = exchange::apply_import(&service, exchange::parse(&text)?, &[keep])?;
    assert_eq!(kept[0].name, "profile 0 (2)");
    let replace = ImportDecision {
        choice: ImportChoice::Replace,
        ..skip
    };
    let replaced = exchange::apply_import(&service, exchange::parse(&text)?, &[replace])?;
    assert_eq!(replaced.len(), 1);
    assert_ne!(replaced[0].id, saved.id);
    let mut changed = saved.clone();
    changed.params.port = 7432;
    let replaced = exchange::apply_import(
        &service,
        exchange::parse(&exchange::export(&[changed])?)?,
        &[replace],
    )?;
    assert_ne!(replaced[0].id, saved.id);
    assert!(service.get(saved.id).is_err());
    Ok(())
}
