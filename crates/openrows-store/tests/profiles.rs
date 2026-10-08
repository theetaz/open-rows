// SPDX-License-Identifier: Apache-2.0
//! Profile CRUD with the in-memory secret store; secrets never hit disk.

use std::{path::Path, sync::Arc};

use openrows_secrets::{MemorySecretStore, SecretStore, SecretString};
use openrows_store::{
    HistoryRecord, HistorySource, HistoryStatus, LocalStore,
    profile::{
        ConnectionProfile, EnvKind, EnvTag, PasswordChange, ProfileError, ProfileFiles,
        ProfileService, SecretRef, ServerParams, parse_url,
    },
};

const CANARY: &str = "Synthetic-Pw-Canary-7c1e";

fn service(dir: &Path) -> (ProfileService, Arc<MemorySecretStore>) {
    let secrets = Arc::new(MemorySecretStore::new());
    let files = ProfileFiles::new(&dir.join("connections"));
    (ProfileService::new(files, secrets.clone()), secrets)
}

fn params(host: &str, database: &str) -> ServerParams {
    ServerParams {
        host: host.to_owned(),
        database: Some(database.to_owned()),
        user: Some("app_user".to_owned()),
        ..ServerParams::default()
    }
}

fn all_bytes_under(dir: &Path) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            bytes.extend(all_bytes_under(&path)?);
        } else {
            bytes.extend(std::fs::read(&path)?);
        }
    }
    Ok(bytes)
}

fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

#[test]
fn create_get_update_delete_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let (service, secrets) = service(dir.path());
    let draft = ConnectionProfile::new_postgres(
        "Orders",
        params("db.internal", "orders"),
        EnvTag::new(EnvKind::Dev),
    );
    let password = SecretString::from(CANARY);
    let created = service.create(draft, Some(&password)).unwrap();
    assert_eq!(created.secrets.password, Some(SecretRef::Keychain));
    assert_eq!(service.get(created.id).unwrap(), created);
    assert_eq!(
        service.password(&created).unwrap().unwrap().expose(),
        CANARY
    );

    let mut edited = created.clone();
    edited.name = "Orders (renamed)".to_owned();
    edited.group = Some("Orders".to_owned());
    let edited = service.update(edited, PasswordChange::Keep).unwrap();
    assert_eq!(edited.secrets.password, Some(SecretRef::Keychain));
    assert_eq!(service.get(created.id).unwrap().name, "Orders (renamed)");

    let cleared = service.update(edited, PasswordChange::Clear).unwrap();
    assert!(cleared.secrets.password.is_none());
    assert!(secrets.get(&cleared.password_key()).unwrap().is_none());

    service.delete(created.id).unwrap();
    assert!(matches!(
        service.get(created.id),
        Err(ProfileError::NotFound { .. })
    ));
    assert!(matches!(
        service.delete(created.id),
        Err(ProfileError::NotFound { .. })
    ));
}

#[test]
fn duplicate_copies_profile_and_password_under_new_id() {
    let dir = tempfile::tempdir().unwrap();
    let (service, _) = service(dir.path());
    let draft =
        ConnectionProfile::new_postgres("Main", params("h", "d"), EnvTag::new(EnvKind::Local));
    let original = service
        .create(draft, Some(&SecretString::from(CANARY)))
        .unwrap();
    let copy = service.duplicate(original.id).unwrap();
    assert_ne!(copy.id, original.id);
    assert_eq!(copy.name, "Main copy");
    assert_eq!(copy.params, original.params);
    assert_eq!(service.password(&copy).unwrap().unwrap().expose(), CANARY);
    service.delete(original.id).unwrap();
    assert_eq!(service.password(&copy).unwrap().unwrap().expose(), CANARY);
}

#[test]
fn list_search_and_groups() {
    let dir = tempfile::tempdir().unwrap();
    let (service, _) = service(dir.path());
    let entries = [
        (
            "Billing prod",
            Some("Billing"),
            "billing.prod",
            EnvKind::Prod,
        ),
        ("Billing dev", Some("Billing"), "billing.dev", EnvKind::Dev),
        ("Scratch", None, "localhost", EnvKind::Local),
    ];
    for (name, group, host, env) in entries {
        let mut profile =
            ConnectionProfile::new_postgres(name, params(host, "app"), EnvTag::new(env));
        profile.group = group.map(str::to_owned);
        service.create(profile, None).unwrap();
    }
    std::fs::write(dir.path().join("connections/broken.json"), "{").unwrap();

    let loaded = service.list().unwrap();
    let names: Vec<_> = loaded.profiles.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Scratch", "Billing dev", "Billing prod"]);
    assert_eq!(loaded.unreadable.len(), 1);
    assert_eq!(service.groups().unwrap(), ["Billing"]);
    let found = service.search("BILLING prod").unwrap();
    assert_eq!(found.len(), 1);
    assert!(found[0].safety.confirm_writes, "prod confirms writes");
    assert_eq!(service.search("").unwrap().len(), 3);
    assert!(service.search("nomatch").unwrap().is_empty());
}

#[test]
fn prod_defaults_to_confirm_writes_including_on_retag() {
    let mut profile = ConnectionProfile::new_postgres(
        "x",
        ServerParams::default(),
        EnvTag::new(EnvKind::Staging),
    );
    assert!(!profile.safety.confirm_writes);
    profile.set_env_tag(EnvTag::new(EnvKind::Prod));
    assert!(profile.safety.confirm_writes);
    let custom = EnvTag::custom("qa", "#123456");
    assert_eq!(custom.kind, EnvKind::Custom);
}

#[test]
fn invalid_profiles_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let (service, _) = service(dir.path());
    let blank =
        ConnectionProfile::new_postgres(" ", ServerParams::default(), EnvTag::new(EnvKind::Dev));
    assert!(matches!(
        service.create(blank, None),
        Err(ProfileError::Invalid { field: "name", .. })
    ));
    let ok =
        ConnectionProfile::new_postgres("a", ServerParams::default(), EnvTag::new(EnvKind::Dev));
    let created = service.create(ok.clone(), None).unwrap();
    assert!(matches!(
        service.create(created, None),
        Err(ProfileError::AlreadyExists { .. })
    ));
}

#[test]
fn secrets_never_reach_profile_files_store_or_debug_output() {
    let dir = tempfile::tempdir().unwrap();
    let (service, secrets) = service(dir.path());
    let store = LocalStore::open(&dir.path().join("openrows.sqlite")).unwrap();
    let url = format!("postgres://app:{CANARY}@db.internal/orders?password={CANARY}x");
    let (draft, password) = parse_url(&url)
        .unwrap()
        .into_profile("Orders", EnvTag::new(EnvKind::Prod));
    let profile = service.create(draft, password.as_ref()).unwrap();
    service
        .update(
            profile.clone(),
            PasswordChange::Set(SecretString::from(format!("{CANARY}-rotated"))),
        )
        .unwrap();
    service.duplicate(profile.id).unwrap();
    store
        .record_history(&HistoryRecord {
            profile_id: profile.id.to_string(),
            database: profile.params.database.clone(),
            started_at_ms: 1,
            duration_ms: None,
            status: HistoryStatus::Ok,
            rows: None,
            statement: "SELECT 1".to_owned(),
            error_message: None,
            source: HistorySource::Editor,
        })
        .unwrap();
    drop(store);

    let on_disk = all_bytes_under(dir.path()).unwrap();
    assert!(on_disk.len() > 1_000, "scan saw the DB and profile files");
    assert!(!contains(&on_disk, CANARY));
    let debug = format!("{profile:?} {password:?} {secrets:?}");
    assert!(!debug.contains(CANARY), "{debug}");
    let stored = secrets.get(&profile.password_key()).unwrap().unwrap();
    assert_eq!(stored.expose(), format!("{CANARY}-rotated"));
}
