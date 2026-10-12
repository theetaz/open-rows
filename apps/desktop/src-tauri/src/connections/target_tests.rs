// SPDX-License-Identifier: Apache-2.0
use super::*;
use openrows_store::profile::{AttachedFile, EnvKind, EnvTag, ServerParams};

fn profile(user: Option<&str>, database: Option<&str>, mode: TlsMode) -> ConnectionProfile {
    let params = ServerParams {
        user: user.map(str::to_owned),
        database: database.map(str::to_owned),
        ..ServerParams::default()
    };
    let mut profile = ConnectionProfile::new_postgres("t", params, EnvTag::new(EnvKind::Local));
    profile.tls = TlsSettings::new(mode);
    profile
}

#[test]
fn database_defaults_to_the_drivers_home_database() -> Result<(), IpcError> {
    let secrets = DriverSecrets {
        password: Some(SecretString::from("synthetic")),
        ..DriverSecrets::default()
    };
    let target = target(&profile(Some("app"), None, TlsMode::Prefer), &secrets)?;
    let request = &target.request;
    assert_eq!(
        (request.user.as_str(), request.database.as_str()),
        ("app", "postgres")
    );
    assert_eq!(
        request.password.as_ref().map(NetSecret::expose),
        Some("synthetic")
    );
    assert_eq!(request.application_name, "OpenRows");
    assert_eq!(request.ssl_mode, SslMode::Prefer);
    Ok(())
}

#[test]
fn tls_files_and_passphrase_reach_the_driver_and_users_are_required() -> Result<(), IpcError> {
    let mut settings = profile(Some("app"), None, TlsMode::VerifyFull);
    settings.tls.ca_file = Some("/certs/ca.pem".into());
    settings.tls.client_key = Some("/certs/client.key".into());
    let secrets = DriverSecrets {
        tls_key_passphrase: Some(SecretString::from("synthetic")),
        ..DriverSecrets::default()
    };
    let target = target(&settings, &secrets)?;
    let files = &target.request.tls;
    assert_eq!(target.request.ssl_mode, SslMode::VerifyFull);
    assert_eq!(
        files.ca_file.as_deref(),
        Some(std::path::Path::new("/certs/ca.pem"))
    );
    assert_eq!(
        files.key_passphrase.as_ref().map(NetSecret::expose),
        Some("synthetic")
    );
    assert!(target.request.via.is_none());
    let missing = super::target(
        &profile(None, Some("db"), TlsMode::Prefer),
        &DriverSecrets::default(),
    );
    assert!(missing.is_err_and(|error| error.message == "User is required"));
    Ok(())
}

#[test]
fn metadata_targets_switch_database_and_name_themselves() -> Result<(), IpcError> {
    let mut base = profile(Some("app"), Some("main"), TlsMode::Disable);
    base.params.application_name = Some("billing".to_owned());
    let metadata = target(&base, &DriverSecrets::default())?
        .with_database("reports")
        .with_application_suffix("metadata");
    assert_eq!(metadata.database(), "reports");
    assert_eq!(metadata.request.application_name, "billing metadata");
    assert_eq!(metadata.request.ssl_mode, SslMode::Disable);
    Ok(())
}

#[test]
fn mongodb_authentication_source_survives_database_switches() -> Result<(), IpcError> {
    let mut base = profile(Some("app"), Some("auth_database"), TlsMode::Disable);
    base.driver = Engine::Mongodb;
    let metadata = target(&base, &DriverSecrets::default())?.with_database("reports");
    assert_eq!(metadata.database(), "reports");
    assert_eq!(
        metadata
            .request
            .driver_options
            .get("authSource")
            .map(String::as_str),
        Some("auth_database")
    );
    base.params
        .options
        .insert("authSource".into(), "explicit".into());
    assert_eq!(
        driver_options(&base).get("authSource").map(String::as_str),
        Some("explicit")
    );
    Ok(())
}

#[test]
fn mongodb_srv_keeps_dns_txt_authentication_source_available() {
    let mut base = profile(None, Some("reports"), TlsMode::VerifyFull);
    base.driver = Engine::Mongodb;
    base.params
        .options
        .insert("mongodb_srv".into(), "true".into());
    assert!(!driver_options(&base).contains_key("authSource"));
}

#[test]
fn network_errors_name_the_endpoint() -> Result<(), IpcError> {
    let target = target(
        &profile(Some("app"), None, TlsMode::Disable),
        &DriverSecrets::default(),
    )?;
    let error = with_endpoint(
        DriverError::Network(std::io::ErrorKind::ConnectionRefused.into()),
        &target.request,
    );
    assert!(error.to_string().contains("localhost:5432"));
    Ok(())
}

#[test]
fn file_profiles_open_their_file_without_a_user() -> Result<(), IpcError> {
    let file = FileParams {
        path: "/data/app.db".into(),
        read_only: true,
        ..FileParams::default()
    };
    let profile = ConnectionProfile::new_file("local", file, EnvTag::new(EnvKind::Local));
    let target = target(&profile, &DriverSecrets::default())?;
    assert_eq!(
        target.request.endpoint,
        Endpoint::File {
            source: FileSource::Disk {
                path: "/data/app.db".into(),
                read_only: true,
                create: false
            },
            attached: Vec::new(),
        }
    );
    assert_eq!(target.database(), "main");
    assert_eq!(target.driver.product(), "SQLite");
    assert_eq!(target.request.describe_endpoint(), "/data/app.db");
    Ok(())
}

#[test]
fn memory_profiles_share_one_named_database_with_their_attachments() -> Result<(), IpcError> {
    let mut file = FileParams::memory();
    file.attach(AttachedFile {
        alias: "logs".to_owned(),
        path: "/data/logs.db".into(),
    })?;
    let profile = ConnectionProfile::new_file("scratch", file, EnvTag::new(EnvKind::Local));
    let target = target(&profile, &DriverSecrets::default())?;
    let expected = Endpoint::File {
        source: FileSource::Memory {
            name: format!("openrows-{}", profile.id),
        },
        attached: vec![Attachment {
            alias: "logs".to_owned(),
            path: "/data/logs.db".into(),
        }],
    };
    assert_eq!(target.request.endpoint, expected);
    assert_eq!(target.request.describe_endpoint(), ":memory:");
    Ok(())
}

#[test]
fn startup_sql_becomes_single_statements_for_every_connect() -> Result<(), IpcError> {
    let mut source = profile(Some("app"), None, TlsMode::Prefer);
    source.startup_commands =
        "SET search_path TO app, public;\n-- note\nSET statement_timeout = '5s';;".to_owned();
    let target = target(&source, &DriverSecrets::default())?;
    assert_eq!(
        target.request.startup_commands,
        [
            "SET search_path TO app, public",
            "-- note\nSET statement_timeout = '5s'"
        ]
    );
    let other = target.with_database("reports");
    assert_eq!(other.request.startup_commands.len(), 2);
    Ok(())
}
