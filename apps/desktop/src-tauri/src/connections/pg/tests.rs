// SPDX-License-Identifier: Apache-2.0
use super::*;
use openrows_driver_postgres::TlsStatus;

fn params(user: Option<&str>, database: Option<&str>) -> PgParams {
    PgParams {
        user: user.map(str::to_owned),
        database: database.map(str::to_owned),
        ..PgParams::default()
    }
}

fn tls(mode: TlsMode) -> TlsSettings {
    TlsSettings::new(mode)
}

#[test]
fn database_defaults_to_the_maintenance_database() -> Result<(), IpcError> {
    let secrets = DriverSecrets {
        password: Some(SecretString::from("synthetic")),
        ..DriverSecrets::default()
    };
    let target = target(&params(Some("app"), None), &tls(TlsMode::Prefer), &secrets)?;
    let config = &target.config;
    assert_eq!(
        (config.user.as_str(), config.database.as_str()),
        ("app", "postgres")
    );
    assert_eq!(config.password.as_deref(), Some("synthetic"));
    assert_eq!(target.options.application_name, "OpenRows");
    assert_eq!(target.options.ssl_mode, SslMode::Prefer);
    Ok(())
}

#[test]
fn tls_files_and_passphrase_reach_the_driver_and_users_are_required() -> Result<(), IpcError> {
    let settings = TlsSettings {
        ca_file: Some("/certs/ca.pem".into()),
        client_cert: Some("/certs/client.pem".into()),
        client_key: Some("/certs/client.key".into()),
        ..tls(TlsMode::VerifyFull)
    };
    let secrets = DriverSecrets {
        tls_key_passphrase: Some(SecretString::from("synthetic")),
        ..DriverSecrets::default()
    };
    let target = target(&params(Some("app"), None), &settings, &secrets)?;
    let files = &target.options.tls;
    assert_eq!(target.options.ssl_mode, SslMode::VerifyFull);
    assert_eq!(
        files.ca_file.as_deref(),
        Some(std::path::Path::new("/certs/ca.pem"))
    );
    assert_eq!(
        files.key_passphrase.as_ref().map(NetSecret::expose),
        Some("synthetic")
    );
    assert!(target.options.via.is_none());
    let user = super::target(
        &params(None, Some("db")),
        &settings,
        &DriverSecrets::default(),
    );
    assert!(user.is_err_and(|error| error.message == "User is required"));
    Ok(())
}

#[test]
fn metadata_targets_switch_database_and_name_themselves() -> Result<(), IpcError> {
    let profile = PgParams {
        application_name: Some("billing".to_owned()),
        ..params(Some("app"), Some("main"))
    };
    let base = target(&profile, &tls(TlsMode::Disable), &DriverSecrets::default())?;
    let metadata = base
        .with_database("reports")
        .with_application_suffix("metadata");
    assert_eq!(metadata.config.database, "reports");
    assert_eq!(metadata.options.application_name, "billing metadata");
    assert_eq!(metadata.options.ssl_mode, SslMode::Disable);
    Ok(())
}

#[test]
fn server_info_names_product_version_and_flavour() {
    let info = ServerInfo {
        server_version: "17.2".to_owned(),
        version: "PostgreSQL 17.2".to_owned(),
        database: "orders".to_owned(),
        user: "app".to_owned(),
        flavour: Flavour::Neon,
        tls: TlsStatus::Disabled,
    };
    let server = ServerDto::from(&info);
    assert_eq!(server.product, "PostgreSQL");
    assert_eq!(server.flavour.as_deref(), Some("Neon"));
    assert_eq!(server.database, "orders");
}

#[test]
fn network_errors_name_the_target() -> Result<(), IpcError> {
    let target = target(
        &params(Some("app"), None),
        &tls(TlsMode::Disable),
        &DriverSecrets::default(),
    )?;
    let error = with_target(
        DriverError::Network(std::io::ErrorKind::ConnectionRefused.into()),
        &target.config,
    );
    assert!(error.to_string().contains("localhost:5432"));
    Ok(())
}
