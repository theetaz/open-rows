// SPDX-License-Identifier: Apache-2.0
//! The profile service touches the secret store only for passwords that
//! exist: on macOS every keychain access can be a password prompt.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use openrows_secrets::{MemorySecretStore, SecretError, SecretKey, SecretStore, SecretString};
use openrows_store::profile::{
    ConnectionProfile, EnvKind, EnvTag, PasswordChange, PgParams, ProfileFiles, ProfileService,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Counts every store call, whatever its kind.
#[derive(Default)]
struct Counting {
    store: MemorySecretStore,
    calls: AtomicUsize,
}

impl Counting {
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn count(&self) {
        self.calls.fetch_add(1, Ordering::SeqCst);
    }
}

impl SecretStore for Counting {
    fn set(&self, key: &SecretKey, value: &SecretString) -> Result<(), SecretError> {
        self.count();
        self.store.set(key, value)
    }

    fn get(&self, key: &SecretKey) -> Result<Option<SecretString>, SecretError> {
        self.count();
        self.store.get(key)
    }

    fn delete(&self, key: &SecretKey) -> Result<(), SecretError> {
        self.count();
        self.store.delete(key)
    }
}

fn service(dir: &tempfile::TempDir) -> (ProfileService, Arc<Counting>) {
    let secrets = Arc::new(Counting::default());
    let files = ProfileFiles::new(&dir.path().join("connections"));
    (ProfileService::new(files, secrets.clone()), secrets)
}

fn draft() -> ConnectionProfile {
    let params = PgParams {
        host: "localhost".to_owned(),
        user: Some("app_user".to_owned()),
        ..PgParams::default()
    };
    ConnectionProfile::new_postgres("Local", params, EnvTag::new(EnvKind::Local))
}

#[test]
fn passwordless_profiles_never_touch_the_store() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (service, secrets) = service(&dir);
    let created = service.create(draft(), None)?;
    assert!(service.password(&created)?.is_none());
    let kept = service.update(created.clone(), PasswordChange::Keep)?;
    service.update(kept, PasswordChange::Clear)?;
    let copy = service.duplicate(created.id)?;
    service.delete(copy.id)?;
    service.delete(created.id)?;
    assert_eq!(secrets.calls(), 0);
    Ok(())
}

#[test]
fn stored_passwords_are_written_once_and_only_removed_when_present() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (service, secrets) = service(&dir);
    let password = SecretString::from("pw-synthetic");
    let created = service.create(draft(), Some(&password))?;
    assert_eq!(secrets.calls(), 1, "create writes the password once");
    let mut renamed = created.clone();
    renamed.name = "Renamed".to_owned();
    let kept = service.update(renamed, PasswordChange::Keep)?;
    assert_eq!(
        secrets.calls(),
        1,
        "keeping the password does not rewrite it"
    );
    assert_eq!(service.password(&kept)?, Some(password));
    let cleared = service.update(kept, PasswordChange::Clear)?;
    assert_eq!(secrets.calls(), 3, "one read, then one delete on clear");
    service.update(cleared, PasswordChange::Clear)?;
    service.delete(created.id)?;
    assert_eq!(secrets.calls(), 3, "nothing left to delete");
    assert!(secrets.store.get(&created.password_key())?.is_none());
    Ok(())
}
