// SPDX-License-Identifier: Apache-2.0
//! Legacy profile migration, portable policies and production-safe copies.
use openrows_secrets::MemorySecretStore;
use openrows_store::profile::{
    ConnectionProfile, EnvKind, EnvTag, ProfileFiles, ProfileService, Safety, ServerParams,
    exchange,
};
use std::sync::Arc;
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn profile(environment: EnvKind) -> ConnectionProfile {
    ConnectionProfile::new_postgres("test", ServerParams::default(), EnvTag::new(environment))
}
#[test]
fn old_profile_booleans_load_and_save_as_one_explicit_policy() -> TestResult {
    let dir = tempfile::tempdir()?;
    let files = ProfileFiles::new(dir.path());
    for (read, confirm, expected) in [
        (false, false, Safety::Unrestricted),
        (false, true, Safety::ConfirmWrites),
        (true, false, Safety::ReadOnly),
        (true, true, Safety::ReadOnly),
    ] {
        let original = profile(EnvKind::Local);
        let mut json = serde_json::to_value(&original)?;
        json["safety"] = serde_json::json!({"readOnly":read,"confirmWrites":confirm});
        std::fs::write(files.path_for(original.id), serde_json::to_vec(&json)?)?;
        let migrated = files.load(original.id)?;
        assert_eq!(migrated.safety, expected);
        files.save(&migrated)?;
        assert!(
            serde_json::from_slice::<serde_json::Value>(&std::fs::read(
                files.path_for(original.id)
            )?)?["safety"]
                .is_string()
        );
    }
    Ok(())
}
#[test]
fn every_policy_round_trips_in_connection_exchange() -> TestResult {
    for safety in [
        Safety::Unrestricted,
        Safety::ConfirmAll,
        Safety::ConfirmWrites,
        Safety::AuthenticateAll,
        Safety::AuthenticateWrites,
        Safety::ReadOnly,
    ] {
        let mut original = profile(EnvKind::Local);
        original.safety = safety;
        let imported = exchange::parse(&exchange::export(&[original])?)?;
        assert_eq!(imported.entries[0].profile.safety, safety);
    }
    Ok(())
}
#[test]
fn production_defaults_imports_and_duplicates_never_silently_become_unrestricted() -> TestResult {
    let mut original = profile(EnvKind::Prod);
    assert_eq!(original.safety, Safety::ConfirmWrites);
    original.safety = Safety::Unrestricted;
    let imported = exchange::parse(&exchange::export(&[original.clone()])?)?;
    assert_eq!(imported.entries[0].profile.safety, Safety::ConfirmWrites);
    assert!(!imported.warnings.is_empty());
    let dir = tempfile::tempdir()?;
    let service = ProfileService::new(
        ProfileFiles::new(dir.path()),
        Arc::new(MemorySecretStore::new()),
    );
    let saved = service.create(original, None)?;
    assert_eq!(service.duplicate(saved.id)?.safety, Safety::ConfirmWrites);
    Ok(())
}
