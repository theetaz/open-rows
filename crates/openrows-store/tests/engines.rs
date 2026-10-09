// SPDX-License-Identifier: Apache-2.0
//! D-12 engines in profiles: `PostgreSQL` files written before D-12 load and
//! save unchanged, `MySQL`/`MariaDB` profiles are server profiles, `SQLite`
//! profiles name a file and need no password.

use std::path::PathBuf;

use openrows_store::profile::{
    AttachedFile, ConnectionProfile, Driver, EnvKind, EnvTag, FileParams, ProfileError,
    ProfileFiles, SecretSlot, ServerParams, SshAuthMethod, SshSettings, TlsMode,
};

/// A profile exactly as D-11 wrote it (format 1, `PostgreSQL` only).
const D11_PROFILE: &str = r##"{
  "openrows": 1,
  "id": "0199b1f2-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
  "name": "Orders",
  "driver": "postgres",
  "envTag": {
    "kind": "dev",
    "color": "#3b82f6"
  },
  "params": {
    "host": "db.internal",
    "port": 5432,
    "database": "orders",
    "user": "app_user"
  },
  "secrets": {
    "password": "keychain"
  },
  "tls": {
    "mode": "verify-full"
  },
  "safety": {
    "readOnly": false,
    "confirmWrites": false
  }
}"##;

#[test]
fn profiles_written_before_d12_load_and_save_unchanged() {
    let profile: ConnectionProfile = serde_json::from_str(D11_PROFILE).unwrap();
    assert_eq!(profile.driver, Driver::Postgres);
    assert!(profile.file.is_none());
    assert!(profile.validate().is_ok());
    let dir = tempfile::tempdir().unwrap();
    let files = ProfileFiles::new(dir.path());
    files.save(&profile).unwrap();
    let written = std::fs::read_to_string(files.path_for(profile.id)).unwrap();
    let reread: serde_json::Value = serde_json::from_str(&written).unwrap();
    let original: serde_json::Value = serde_json::from_str(D11_PROFILE).unwrap();
    assert_eq!(reread, original);
}

#[test]
fn mysql_profiles_are_server_profiles() {
    let profile = ConnectionProfile::new(
        "Shop",
        Driver::Mariadb,
        ServerParams::local(3306),
        EnvTag::new(EnvKind::Local),
    );
    assert!(profile.validate().is_ok());
    assert_eq!(profile.tls.mode, TlsMode::Prefer);
    assert!(profile.uses_secret(SecretSlot::Password));
    let json = serde_json::to_value(&profile).unwrap();
    assert_eq!(json["driver"], "mariadb");
    assert_eq!(Driver::Mysql.default_port(), Some(3306));
}

fn sqlite(path: &str) -> ConnectionProfile {
    ConnectionProfile::new_file(
        "Local",
        FileParams {
            path: PathBuf::from(path),
            read_only: true,
            ..FileParams::default()
        },
        EnvTag::new(EnvKind::Local),
    )
}

#[test]
fn sqlite_profiles_name_an_absolute_file_without_route_or_password() {
    let profile = sqlite("/data/app.db");
    assert!(profile.validate().is_ok());
    assert_eq!(profile.tls.mode, TlsMode::Disable);
    assert!(!profile.uses_secret(SecretSlot::Password));
    let json = serde_json::to_value(&profile).unwrap();
    assert_eq!(json["driver"], "sqlite");
    assert_eq!(json["file"]["path"], "/data/app.db");
    assert_eq!(json["file"]["readOnly"], true);
    let back: ConnectionProfile = serde_json::from_value(json).unwrap();
    assert_eq!(back, profile);
    let invalid = |profile: &ConnectionProfile| {
        matches!(profile.validate(), Err(ProfileError::Invalid { .. }))
    };
    assert!(invalid(&sqlite("relative.db")));
    assert!(invalid(&sqlite("")));
    let mut tunnelled = sqlite("/data/app.db");
    tunnelled.ssh = Some(SshSettings {
        host: "bastion".to_owned(),
        port: None,
        user: None,
        auth: SshAuthMethod::Agent,
        jump_hosts: Vec::new(),
    });
    assert!(invalid(&tunnelled));
    let mut missing = sqlite("/data/app.db");
    missing.file = None;
    assert!(invalid(&missing));
}

#[test]
fn in_memory_profiles_need_no_path_and_keep_valid_attachments() {
    let mut profile =
        ConnectionProfile::new_file("Scratch", FileParams::memory(), EnvTag::new(EnvKind::Local));
    assert!(profile.validate().is_ok());
    let file = profile.file.as_mut().unwrap();
    file.attach(AttachedFile {
        alias: "logs".to_owned(),
        path: PathBuf::from("/data/logs.db"),
    })
    .unwrap();
    let json = serde_json::to_value(&profile).unwrap();
    assert_eq!(json["file"]["memory"], true);
    assert_eq!(json["file"]["attached"][0]["alias"], "logs");
    assert!(json["file"].get("readOnly").is_none());
    let back: ConnectionProfile = serde_json::from_value(json).unwrap();
    assert_eq!(back, profile);
    let older: FileParams = serde_json::from_str(r#"{"path":"/data/app.db"}"#).unwrap();
    assert!(!older.memory && older.attached.is_empty());
    profile.file.as_mut().unwrap().attached.push(AttachedFile {
        alias: "main".to_owned(),
        path: PathBuf::from("/data/other.db"),
    });
    assert!(matches!(
        profile.validate(),
        Err(ProfileError::Invalid {
            field: "file.attached.alias",
            ..
        })
    ));
}
