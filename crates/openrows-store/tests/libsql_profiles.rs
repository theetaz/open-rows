// SPDX-License-Identifier: Apache-2.0
//! Remote token policies, safe exchange and backward-compatible file profiles.
use openrows_secrets::{MemorySecretStore, SecretStore, SecretString};
use openrows_store::profile::{
    Driver, EnvKind, EnvTag, PasswordChange, PasswordStorage, ProfileFiles, ProfileService,
    TlsMode, exchange, parse_url,
};
use std::sync::Arc;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn libsql_urls_exclude_credentials_and_keep_file_flavour() -> TestResult {
    for url in ["libsql://db.test", "https://db.test/", "LIBSQL://db.test"] {
        let parsed = parse_url(url)?;
        assert_eq!(parsed.driver, Driver::Libsql);
        assert_eq!(parsed.params.port, 443);
        assert_eq!(parsed.tls_mode, TlsMode::VerifyFull);
        assert!(parsed.password.is_none());
    }
    for url in [
        "libsql://u:test-only@db.test",
        "https://db.test/?token=test-only",
        "https://db.test/test-only",
        "libsql://db.test#test-only",
    ] {
        let error = parse_url(url).err().ok_or("credentials accepted")?;
        assert!(!format!("{error:?} {error}").contains("test-only"));
    }
    let parsed = parse_url("http://127.0.0.1:8080")?;
    assert_eq!(parsed.tls_mode, TlsMode::Disable);
    for url in ["libsql-file:/data/app.db", "libsql-file::memory:"] {
        let (profile, secret) = parse_url(url)?.into_profile("file", EnvTag::new(EnvKind::Local));
        assert_eq!(profile.driver, Driver::LibsqlFile);
        profile.validate()?;
        assert!(secret.is_none());
    }
    assert_eq!(parse_url("sqlite::memory:")?.driver, Driver::Sqlite);
    Ok(())
}

#[test]
fn libsql_token_keychain_ask_and_exports_never_persist_values() -> TestResult {
    let dir = tempfile::tempdir_in(".")?;
    let secrets = Arc::new(MemorySecretStore::new());
    let service = ProfileService::new(ProfileFiles::new(dir.path()), secrets.clone());
    let (draft, _) =
        parse_url("libsql://db.test")?.into_profile("remote", EnvTag::new(EnvKind::Dev));
    let profile = service.create(draft, Some(&SecretString::from("test-only")))?;
    assert_eq!(
        service.password(&profile)?.ok_or("token missing")?.expose(),
        "test-only"
    );
    let exported = exchange::export(std::slice::from_ref(&profile))?;
    assert!(!exported.contains("test-only"));
    assert!(!format!("{profile:?}").contains("test-only"));
    let imported = exchange::parse(&exported)?;
    assert_eq!(imported.entries[0].profile.driver, Driver::Libsql);
    for entry in std::fs::read_dir(dir.path())? {
        assert!(!String::from_utf8_lossy(&std::fs::read(entry?.path())?).contains("test-only"));
    }
    let mut ask = profile;
    ask.password_storage = PasswordStorage::Ask;
    let ask = service.update(ask, PasswordChange::Keep)?;
    assert!(service.password(&ask)?.is_none());
    assert!(secrets.get(&ask.password_key())?.is_none());
    Ok(())
}

#[test]
fn libsql_exchange_preserves_flavours_helpers_and_history_without_tokens() -> TestResult {
    use openrows_store::profile::HelperHop;
    for url in [
        "libsql://db.test",
        "libsql-file::memory:",
        "clickhouse://db.test",
    ] {
        let (mut profile, _) = parse_url(url)?.into_profile("test", EnvTag::new(EnvKind::Dev));
        profile.redact_history = true;
        profile
            .params
            .options
            .insert("token".into(), "test-only".into());
        if !profile.driver.is_file() {
            profile.helper = Some(HelperHop::Custom {
                program: "/test/helper".into(),
                args: vec!["{local_port}".into(), "{remote_host}:{remote_port}".into()],
                ready_timeout_seconds: None,
            });
        }
        let exported = exchange::export(std::slice::from_ref(&profile))?;
        assert!(!exported.contains("test-only"));
        let imported = exchange::parse(&exported)?;
        let entry = &imported.entries[0];
        assert_eq!(entry.profile.driver, profile.driver);
        assert_eq!(entry.profile.helper, profile.helper);
        assert!(entry.profile.redact_history);
        assert!(entry.profile.imported);
        assert_ne!(entry.profile.id, profile.id);
        assert!(entry.password.is_none());
        assert!(entry.profile.secrets.password.is_none());
    }
    Ok(())
}

#[test]
fn libsql_remote_accepts_tcp_helpers_and_file_rejects_them() -> TestResult {
    use openrows_store::profile::HelperHop;
    for url in ["libsql://db.test", "libsql-file::memory:"] {
        let (mut profile, _) = parse_url(url)?.into_profile("test", EnvTag::new(EnvKind::Dev));
        for helper in [
            HelperHop::Kubectl {
                context: "test".into(),
                namespace: "default".into(),
                resource: "svc/db".into(),
                remote_port: 443,
                kubeconfig: None,
            },
            HelperHop::Custom {
                program: "/test/helper".into(),
                args: vec!["{local_port}".into()],
                ready_timeout_seconds: None,
            },
        ] {
            profile.helper = Some(helper);
            assert_eq!(profile.validate().is_ok(), profile.driver == Driver::Libsql);
        }
    }
    Ok(())
}
