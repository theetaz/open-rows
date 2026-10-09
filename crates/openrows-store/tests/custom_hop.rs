// SPDX-License-Identifier: Apache-2.0
//! Purpose separation, edit invalidation, and untrusted custom-hop exchange.
use openrows_store::profile::{
    CommandPurpose, CommandTrust, ConnectionProfile, CredentialError, EnvKind, EnvTag, HelperHop,
    ServerParams, acquire_trusted_hop, exchange, hop_command, trusted_hop_spec,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn profile(marker: &std::path::Path) -> ConnectionProfile {
    let mut profile = ConnectionProfile::new_postgres(
        "custom",
        ServerParams::default(),
        EnvTag::new(EnvKind::Local),
    );
    profile.helper = Some(HelperHop::Custom {
        program: "/usr/bin/touch".into(),
        args: vec![marker.to_string_lossy().into_owned(), "{local_port}".into()],
        ready_timeout_seconds: Some(1),
    });
    profile
}
#[tokio::test]
async fn untrusted_custom_hop_never_spawns_and_purposes_are_isolated() -> TestResult {
    let dir = tempfile::tempdir()?;
    let marker = dir.path().join("started");
    let profile = profile(&marker);
    let command = hop_command(&profile).ok_or("command")?;
    let pool = openrows_net::helper::Helpers::default();
    let trust = CommandTrust::new(&dir.path().join("trust.json"));
    assert!(matches!(
        acquire_trusted_hop(&pool, &profile, &trust).await,
        Err(CredentialError::Untrusted)
    ));
    assert!(!marker.exists());
    trust.trust(profile.id, &command)?;
    assert!(matches!(
        acquire_trusted_hop(&pool, &profile, &trust).await,
        Err(CredentialError::Untrusted)
    ));
    assert!(!marker.exists());
    let other = CommandTrust::new(&dir.path().join("hop-trust.json"));
    other.trust_for(profile.id, CommandPurpose::Hop, &command)?;
    assert!(!other.is_trusted(profile.id, &command)?);
    assert!(matches!(
        other.require(profile.id, &command),
        Err(CredentialError::Untrusted)
    ));
    assert!(trusted_hop_spec(&profile, &other)?.is_some());
    for changed in [
        HelperHop::Custom {
            program: "/other".into(),
            args: command.args.clone(),
            ready_timeout_seconds: None,
        },
        HelperHop::Custom {
            program: command.program.to_string_lossy().into_owned(),
            args: vec!["changed{local_port}".into()],
            ready_timeout_seconds: None,
        },
    ] {
        let mut edited = profile.clone();
        edited.helper = Some(changed);
        assert!(matches!(
            trusted_hop_spec(&edited, &other),
            Err(CredentialError::Untrusted)
        ));
    }
    Ok(())
}
#[test]
fn custom_hop_exchange_round_trip_has_fresh_untrusted_id() -> TestResult {
    let dir = tempfile::tempdir()?;
    let profile = profile(&dir.path().join("started"));
    let trust = CommandTrust::new(&dir.path().join("trust.json"));
    trust.trust_for(
        profile.id,
        CommandPurpose::Hop,
        &hop_command(&profile).ok_or("command")?,
    )?;
    let exported = exchange::export(std::slice::from_ref(&profile))?;
    assert!(exported.contains("readyTimeoutSeconds"));
    let batch = exchange::parse(&exported)?;
    assert!(batch.warnings.is_empty());
    let imported = &batch.entries[0].profile;
    assert_eq!(imported.helper, profile.helper);
    assert!(imported.imported);
    assert_ne!(profile.id, imported.id);
    assert!(matches!(
        trusted_hop_spec(imported, &trust),
        Err(CredentialError::Untrusted)
    ));
    assert!(!dir.path().join("started").exists());
    Ok(())
}
