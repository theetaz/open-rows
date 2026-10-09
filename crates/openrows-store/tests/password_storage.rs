// SPDX-License-Identifier: Apache-2.0
//! Keychain or "ask every time" passwords (docs/12 §3): asked passwords are
//! never stored, and files written before the choice keep the keychain.

use std::{path::Path, sync::Arc};

use openrows_secrets::{MemorySecretStore, SecretStore, SecretString};
use openrows_store::profile::{
    ConnectionProfile, EnvKind, EnvTag, PasswordChange, PasswordStorage, ProfileFiles,
    ProfileService, SecretRef, ServerParams,
};

const CANARY: &str = "Synthetic-Ask-Canary-41b2";

fn service(dir: &Path) -> (ProfileService, Arc<MemorySecretStore>) {
    let secrets = Arc::new(MemorySecretStore::new());
    let files = ProfileFiles::new(dir);
    (ProfileService::new(files, secrets.clone()), secrets)
}

fn profile(name: &str) -> ConnectionProfile {
    let params = ServerParams {
        host: "db.internal".to_owned(),
        user: Some("app".to_owned()),
        ..ServerParams::default()
    };
    ConnectionProfile::new_postgres(name, params, EnvTag::new(EnvKind::Dev))
}

#[test]
fn files_without_the_choice_keep_the_keychain() {
    let dir = tempfile::tempdir().unwrap();
    let (service, _) = service(dir.path());
    let created = service.create(profile("Legacy"), None).unwrap();
    let path = dir.path().join(format!("{}.json", created.id));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("passwordStorage"), "{text}");
    assert_eq!(
        service.get(created.id).unwrap().password_storage,
        PasswordStorage::Keychain
    );
}

#[test]
fn asked_passwords_are_never_stored() {
    let dir = tempfile::tempdir().unwrap();
    let (service, secrets) = service(dir.path());
    let mut draft = profile("Ask");
    draft.password_storage = PasswordStorage::Ask;
    let created = service
        .create(draft, Some(&SecretString::from(CANARY)))
        .unwrap();
    assert_eq!(created.secrets.password, None);
    assert!(secrets.get(&created.password_key()).unwrap().is_none());
    assert!(service.password(&created).unwrap().is_none());
    let path = dir.path().join(format!("{}.json", created.id));
    let text = std::fs::read_to_string(path).unwrap();
    assert!(text.contains("\"passwordStorage\": \"ask\""), "{text}");
    assert!(!text.contains(CANARY));
}

#[test]
fn switching_to_ask_forgets_the_saved_password() {
    let dir = tempfile::tempdir().unwrap();
    let (service, secrets) = service(dir.path());
    let created = service
        .create(profile("Switch"), Some(&SecretString::from(CANARY)))
        .unwrap();
    assert_eq!(created.secrets.password, Some(SecretRef::Keychain));
    let mut edited = created.clone();
    edited.password_storage = PasswordStorage::Ask;
    let edited = service.update(edited, PasswordChange::Keep).unwrap();
    assert_eq!(edited.secrets.password, None);
    assert!(secrets.get(&created.password_key()).unwrap().is_none());
}

#[test]
fn search_matches_the_environment_kind() {
    let dir = tempfile::tempdir().unwrap();
    let (service, _) = service(dir.path());
    let mut prod = profile("Billing");
    prod.set_env_tag(EnvTag::new(EnvKind::Prod));
    service.create(prod, None).unwrap();
    service.create(profile("Scratch"), None).unwrap();
    let found = service.search("prod").unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "Billing");
}
