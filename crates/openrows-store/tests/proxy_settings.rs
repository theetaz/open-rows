// SPDX-License-Identifier: Apache-2.0
//! Proxy and cloud CA profile settings (M1-06): serde, validation, the
//! insecure-connection flag and the proxy password's keychain lifecycle.
#![allow(
    clippy::unwrap_used,
    reason = "tests fail fast on fixture errors, like the other store tests"
)]

use std::{path::PathBuf, sync::Arc};

use openrows_secrets::{MemorySecretStore, SecretStore, SecretString};
use openrows_store::profile::{
    CloudCa, ConnectionProfile, EnvKind, EnvTag, Insecurity, PasswordChange, ProfileError,
    ProfileFiles, ProfileService, ProxyKind, ProxySettings, SecretChanges, SecretRef, SecretSlot,
    ServerParams, SshAuthMethod, SshSettings, TlsMode, TlsSettings,
};

const PROXY_CANARY: &str = "Synthetic-Proxy-Canary-7c1e";

fn profile(host: &str) -> ConnectionProfile {
    let params = ServerParams {
        host: host.to_owned(),
        user: Some("app".to_owned()),
        ..ServerParams::default()
    };
    ConnectionProfile::new_postgres("Orders", params, EnvTag::new(EnvKind::Dev))
}

fn proxy(user: Option<&str>) -> ProxySettings {
    ProxySettings {
        kind: ProxyKind::Socks5,
        host: "proxy.corp".to_owned(),
        port: 1080,
        user: user.map(str::to_owned),
    }
}

fn invalid_field(profile: &ConnectionProfile) -> Option<&'static str> {
    match profile.validate() {
        Err(ProfileError::Invalid { field, .. }) => Some(field),
        _ => None,
    }
}

#[test]
fn proxy_and_cloud_ca_round_trip_without_secrets() {
    let mut profile = profile("db.example.com");
    profile.proxy = Some(proxy(Some("ops")));
    profile.tls.cloud_ca = Some(CloudCa::AwsRds);
    let json = serde_json::to_value(&profile).unwrap();
    assert_eq!(
        json["proxy"],
        serde_json::json!({ "kind": "socks5", "host": "proxy.corp", "port": 1080, "user": "ops" })
    );
    assert_eq!(json["tls"]["cloudCa"], "awsRds");
    let back: ConnectionProfile = serde_json::from_value(json).unwrap();
    assert_eq!(back, profile);
    let direct = serde_json::to_value(self::profile("db")).unwrap();
    assert!(direct.get("proxy").is_none() && direct["tls"].get("cloudCa").is_none());
}

#[test]
fn proxies_are_validated() {
    let mut profile = profile("db.example.com");
    profile.proxy = Some(ProxySettings {
        host: " ".to_owned(),
        ..proxy(None)
    });
    assert_eq!(invalid_field(&profile), Some("proxy.host"));
    profile.proxy = Some(ProxySettings {
        port: 0,
        ..proxy(None)
    });
    assert_eq!(invalid_field(&profile), Some("proxy.port"));
    profile.proxy = Some(proxy(Some(" ")));
    assert_eq!(invalid_field(&profile), Some("proxy.user"));
    let mut socket = self::profile("/var/run/postgresql");
    socket.proxy = Some(proxy(None));
    assert_eq!(
        invalid_field(&socket),
        Some("proxy"),
        "a proxy cannot reach a local socket"
    );
    socket.ssh = Some(SshSettings {
        host: "bastion".to_owned(),
        port: None,
        user: None,
        auth: SshAuthMethod::Agent,
        jump_hosts: Vec::new(),
    });
    assert_eq!(
        invalid_field(&socket),
        None,
        "the socket is on the SSH server"
    );
    profile.proxy = None;
    profile.tls.ca_file = Some(PathBuf::from("/certs/ca.pem"));
    profile.tls.cloud_ca = Some(CloudCa::Azure);
    assert_eq!(invalid_field(&profile), Some("tls.caFile"));
}

#[test]
fn insecure_connections_are_flagged_for_remote_hosts_only() {
    let tls = |mode| TlsSettings::new(mode);
    assert_eq!(
        tls(TlsMode::Disable).insecurity("db.example.com"),
        Some(Insecurity::TlsDisabled)
    );
    assert_eq!(
        tls(TlsMode::Require).insecurity("db.example.com"),
        Some(Insecurity::TlsUnverified)
    );
    assert_eq!(tls(TlsMode::VerifyFull).insecurity("db.example.com"), None);
    assert_eq!(tls(TlsMode::Disable).insecurity("localhost"), None);
    assert_eq!(tls(TlsMode::Disable).insecurity("/tmp"), None);
    let mut cloud = tls(TlsMode::Require);
    cloud.cloud_ca = Some(CloudCa::AwsRds);
    assert!(
        !cloud.skips_verification(),
        "require + a CA verifies the chain"
    );
    assert_eq!(cloud.insecurity("db.example.com"), None);
}

#[test]
fn the_proxy_password_lives_in_the_keychain_while_a_proxy_user_is_set() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::new());
    let files = ProfileFiles::new(&dir.path().join("connections"));
    let service = ProfileService::new(files, secrets.clone());
    let mut draft = profile("db.example.com");
    draft.proxy = Some(proxy(Some("ops")));
    let changes = SecretChanges {
        proxy_password: PasswordChange::Set(SecretString::from(PROXY_CANARY)),
        ..SecretChanges::default()
    };
    let created = service.create_with(draft, &changes).unwrap();
    assert_eq!(created.secrets.proxy_password, Some(SecretRef::Keychain));
    let stored = service.secret(&created, SecretSlot::ProxyPassword).unwrap();
    assert_eq!(stored.unwrap().expose(), PROXY_CANARY);
    let file = std::fs::read_to_string(dir.path().join(format!("connections/{}.json", created.id)))
        .unwrap();
    assert!(!file.contains(PROXY_CANARY));
    let mut anonymous = created.clone();
    anonymous.proxy = Some(proxy(None));
    let updated = service
        .update_with(anonymous, &SecretChanges::default())
        .unwrap();
    assert_eq!(updated.secrets.proxy_password, None, "no user, no password");
    let key = created.secret_key(SecretSlot::ProxyPassword);
    assert!(secrets.get(&key).unwrap().is_none());
}
