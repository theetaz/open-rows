// SPDX-License-Identifier: Apache-2.0
//! SSH and TLS profile settings (D-10, D-11): serde back-compat, validation and
//! the keychain lifecycle of SSH and TLS secrets.
#![allow(
    clippy::unwrap_used,
    reason = "tests fail fast on fixture errors, like the other store tests"
)]

use std::{path::PathBuf, sync::Arc};

use openrows_secrets::{MemorySecretStore, SecretStore, SecretString};
use openrows_store::profile::{
    ConnectionProfile, EnvKind, EnvTag, PasswordChange, PgParams, ProfileError, ProfileFiles,
    ProfileService, SecretChanges, SecretRef, SecretSlot, SshAuthMethod, SshSettings, TlsMode,
    TlsSettings,
};

const SSH_CANARY: &str = "Synthetic-Ssh-Canary-41d2";
const KEY_CANARY: &str = "Synthetic-Key-Canary-9b3f";

/// A profile file as D-03 to D-09 wrote it: no `ssh`, TLS mode only.
const PRE_D10_PROFILE: &str = r##"{
  "openrows": 1,
  "id": "0192f0c4-8a5e-7c2b-9d1e-3f4a5b6c7d8e",
  "name": "Orders",
  "driver": "postgres",
  "envTag": { "kind": "dev", "color": "#2f9e44" },
  "params": { "host": "db.internal", "port": 5432, "user": "app" },
  "secrets": { "password": "keychain" },
  "tls": { "mode": "verify-full" },
  "safety": { "readOnly": false, "confirmWrites": false }
}"##;

fn service() -> (tempfile::TempDir, ProfileService, Arc<MemorySecretStore>) {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::new());
    let files = ProfileFiles::new(&dir.path().join("connections"));
    let service = ProfileService::new(files, secrets.clone());
    (dir, service, secrets)
}

fn ssh(auth: SshAuthMethod) -> SshSettings {
    SshSettings {
        host: "bastion".to_owned(),
        port: Some(2222),
        user: Some("ops".to_owned()),
        auth,
        jump_hosts: vec!["edge@jump.example.com:22".to_owned()],
    }
}

fn profile() -> ConnectionProfile {
    let params = PgParams {
        host: "db.internal".to_owned(),
        user: Some("app".to_owned()),
        ..PgParams::default()
    };
    ConnectionProfile::new_postgres("Orders", params, EnvTag::new(EnvKind::Dev))
}

fn set(value: &str) -> PasswordChange {
    PasswordChange::Set(SecretString::from(value))
}

#[test]
fn profiles_without_ssh_or_tls_files_still_load_and_round_trip() {
    let loaded: ConnectionProfile = serde_json::from_str(PRE_D10_PROFILE).unwrap();
    assert_eq!(loaded.ssh, None);
    assert_eq!(loaded.tls, TlsSettings::new(TlsMode::VerifyFull));
    assert_eq!(loaded.secrets.password, Some(SecretRef::Keychain));
    assert_eq!(loaded.secrets.ssh_password, None);
    let written = serde_json::to_value(&loaded).unwrap();
    assert!(written.get("ssh").is_none(), "absent SSH is not written");
    assert_eq!(written["tls"], serde_json::json!({ "mode": "verify-full" }));
}

#[test]
fn ssh_and_tls_settings_serialise_without_secrets() {
    let mut profile = profile();
    profile.ssh = Some(ssh(SshAuthMethod::PrivateKey {
        key_file: PathBuf::from("~/.ssh/id_ed25519"),
    }));
    profile.tls.ca_file = Some(PathBuf::from("/certs/ca.pem"));
    let json = serde_json::to_value(&profile).unwrap();
    assert_eq!(json["ssh"]["auth"]["method"], "privateKey");
    assert_eq!(json["ssh"]["auth"]["keyFile"], "~/.ssh/id_ed25519");
    assert_eq!(json["ssh"]["jumpHosts"][0], "edge@jump.example.com:22");
    assert_eq!(json["tls"]["caFile"], "/certs/ca.pem");
    let back: ConnectionProfile = serde_json::from_value(json).unwrap();
    assert_eq!(back, profile);
}

#[test]
fn ssh_settings_are_validated() {
    let mut profile = profile();
    profile.ssh = Some(SshSettings {
        host: " ".to_owned(),
        ..ssh(SshAuthMethod::Agent)
    });
    assert!(matches!(
        profile.validate(),
        Err(ProfileError::Invalid {
            field: "ssh.host",
            ..
        })
    ));
    profile.ssh = Some(SshSettings {
        port: Some(0),
        ..ssh(SshAuthMethod::Agent)
    });
    assert!(matches!(
        profile.validate(),
        Err(ProfileError::Invalid {
            field: "ssh.port",
            ..
        })
    ));
    profile.ssh = Some(ssh(SshAuthMethod::PrivateKey {
        key_file: PathBuf::new(),
    }));
    assert!(matches!(
        profile.validate(),
        Err(ProfileError::Invalid {
            field: "ssh.keyFile",
            ..
        })
    ));
}

#[test]
fn skip_verification_follows_libpq() {
    let mut tls = TlsSettings::new(TlsMode::Require);
    assert!(tls.skips_verification());
    tls.ca_file = Some(PathBuf::from("/certs/ca.pem"));
    assert!(
        !tls.skips_verification(),
        "require with a CA verifies the chain"
    );
    assert!(TlsSettings::new(TlsMode::Prefer).skips_verification());
    assert!(!TlsSettings::new(TlsMode::VerifyFull).skips_verification());
}

#[test]
fn ssh_and_tls_secrets_live_in_the_keychain_and_follow_the_settings() {
    let (dir, service, secrets) = service();
    let mut draft = profile();
    draft.ssh = Some(ssh(SshAuthMethod::Password));
    draft.tls.client_cert = Some(PathBuf::from("/certs/client.pem"));
    draft.tls.client_key = Some(PathBuf::from("/certs/client.key"));
    let changes = SecretChanges {
        ssh_password: set(SSH_CANARY),
        ssh_passphrase: set("ignored: password auth has no key"),
        tls_key_passphrase: set(KEY_CANARY),
        ..SecretChanges::default()
    };
    let created = service.create_with(draft, &changes).unwrap();
    assert_eq!(created.secrets.ssh_password, Some(SecretRef::Keychain));
    assert_eq!(created.secrets.ssh_passphrase, None);
    let read = |slot| {
        service
            .secret(&created, slot)
            .unwrap()
            .map(|s| s.expose().to_owned())
    };
    assert_eq!(read(SecretSlot::SshPassword).as_deref(), Some(SSH_CANARY));
    assert_eq!(
        read(SecretSlot::TlsKeyPassphrase).as_deref(),
        Some(KEY_CANARY)
    );
    let file = std::fs::read_to_string(dir.path().join(format!("connections/{}.json", created.id)))
        .unwrap();
    assert!(!file.contains(SSH_CANARY) && !file.contains(KEY_CANARY));

    let copy = service.duplicate(created.id).unwrap();
    let copied = service.secret(&copy, SecretSlot::SshPassword).unwrap();
    assert_eq!(
        copied.map(|s| s.expose().to_owned()).as_deref(),
        Some(SSH_CANARY)
    );

    let mut direct = created.clone();
    direct.ssh = None;
    direct.tls.client_key = None;
    direct.tls.client_cert = None;
    let updated = service
        .update_with(direct, &SecretChanges::default())
        .unwrap();
    assert_eq!(
        updated.secrets.ssh_password, None,
        "turning SSH off forgets its password"
    );
    assert_eq!(updated.secrets.tls_key_passphrase, None);
    let ssh_key = created.secret_key(SecretSlot::SshPassword);
    assert!(secrets.get(&ssh_key).unwrap().is_none());

    service.delete(copy.id).unwrap();
    assert!(
        secrets
            .get(&copy.secret_key(SecretSlot::SshPassword))
            .unwrap()
            .is_none()
    );
}

#[test]
fn keeping_a_secret_never_rewrites_it() {
    let (_dir, service, secrets) = service();
    let mut draft = profile();
    draft.ssh = Some(ssh(SshAuthMethod::PrivateKey {
        key_file: PathBuf::from("~/.ssh/id_ed25519"),
    }));
    let changes = SecretChanges {
        ssh_passphrase: set(KEY_CANARY),
        ..SecretChanges::default()
    };
    let created = service.create_with(draft, &changes).unwrap();
    let mut renamed = created.clone();
    renamed.name = "Renamed".to_owned();
    let kept = service
        .update_with(renamed, &SecretChanges::default())
        .unwrap();
    assert_eq!(kept.secrets.ssh_passphrase, Some(SecretRef::Keychain));
    let key = created.secret_key(SecretSlot::SshPassphrase);
    assert_eq!(
        secrets
            .get(&key)
            .unwrap()
            .map(|s| s.expose().to_owned())
            .as_deref(),
        Some(KEY_CANARY)
    );
    let cleared = SecretChanges {
        ssh_passphrase: PasswordChange::Clear,
        ..SecretChanges::default()
    };
    let forgotten = service.update_with(kept, &cleared).unwrap();
    assert_eq!(forgotten.secrets.ssh_passphrase, None);
    assert!(secrets.get(&key).unwrap().is_none());
}
