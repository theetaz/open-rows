// SPDX-License-Identifier: Apache-2.0
//! Translate public client options into engine profiles, never into opaque secret options.
use super::ini::sections;
use crate::profile::{
    ConnectionProfile, Driver, EnvKind, EnvTag, ServerParams, TlsMode,
    exchange::{ExchangeError, ImportBatch, ImportEntry},
};
use openrows_secrets::SecretString;

/// Convert libpq service sections into `PostgreSQL` profiles.
/// # Errors
/// Invalid port, TLS mode or concrete profile settings.
pub fn parse_pg_service(text: &str) -> Result<ImportBatch, ExchangeError> {
    parse_sections(text, false)
}
/// Convert `MySQL` client, mysql and client-suffix groups; includes stay ignored.
/// # Errors
/// Invalid port, TLS mode or concrete profile settings.
pub fn parse_my_cnf(text: &str) -> Result<ImportBatch, ExchangeError> {
    parse_sections(text, true)
}
fn parse_sections(text: &str, mysql: bool) -> Result<ImportBatch, ExchangeError> {
    let mut batch = ImportBatch::default();
    for section in sections(text, mysql, &mut batch) {
        if mysql && section.name != "mysql" && !section.name.starts_with("client") {
            batch
                .warnings
                .push(format!("Group [{}] ignored", section.name));
            continue;
        }
        let mut profile = ConnectionProfile::new(
            &section.name,
            if mysql {
                Driver::Mysql
            } else {
                Driver::Postgres
            },
            ServerParams::local(if mysql { 3306 } else { 5432 }),
            EnvTag::new(EnvKind::Local),
        );
        profile.imported = true;
        let mut password = None;
        // Host defaults must be decided before the default TLS mode.
        if let Some(host) = section.values.get("host") {
            profile.params.host = host.to_string();
        }
        profile.tls.mode = TlsMode::default_for_host(&profile.params.host);
        for (key, value) in &section.values {
            if key == "password" {
                password = Some(SecretString::from(value.as_str()));
            } else if !apply(&mut profile, key, value, mysql)? {
                batch
                    .warnings
                    .push(format!("[{}]: ignored key {key}", section.name));
            }
        }
        profile.validate()?;
        batch.entries.push(ImportEntry { profile, password });
    }
    Ok(batch)
}
fn apply(
    profile: &mut ConnectionProfile,
    key: &str,
    value: &str,
    mysql: bool,
) -> Result<bool, ExchangeError> {
    match key {
        "host" => value.clone_into(&mut profile.params.host),
        "port" => {
            profile.params.port = value
                .parse()
                .map_err(|_| ExchangeError::Invalid("invalid client port".into()))?;
        }
        "user" => profile.params.user = Some(value.to_owned()),
        "dbname" if !mysql => profile.params.database = Some(value.to_owned()),
        "database" if mysql => profile.params.database = Some(value.to_owned()),
        "application_name" if !mysql => profile.params.application_name = Some(value.to_owned()),
        "connect_timeout" if !mysql => {
            profile
                .params
                .options
                .insert(key.to_owned(), value.to_owned());
        }
        "socket" if mysql => value.clone_into(&mut profile.params.host),
        "sslmode" if !mysql => profile.tls.mode = tls_mode(value)?,
        "ssl-mode" if mysql => profile.tls.mode = tls_mode(value)?,
        "sslrootcert" if !mysql => profile.tls.ca_file = Some(value.into()),
        "ssl-ca" if mysql => profile.tls.ca_file = Some(value.into()),
        "sslcert" if !mysql => profile.tls.client_cert = Some(value.into()),
        "ssl-cert" if mysql => profile.tls.client_cert = Some(value.into()),
        "sslkey" if !mysql => profile.tls.client_key = Some(value.into()),
        "ssl-key" if mysql => profile.tls.client_key = Some(value.into()),
        _ => return Ok(false),
    }
    Ok(true)
}
fn tls_mode(value: &str) -> Result<TlsMode, ExchangeError> {
    let lower = value.to_ascii_lowercase();
    let mode = match lower.as_str() {
        "disabled" => Some(TlsMode::Disable),
        "preferred" => Some(TlsMode::Prefer),
        "required" => Some(TlsMode::Require),
        "verify_ca" => Some(TlsMode::VerifyCa),
        "verify_identity" => Some(TlsMode::VerifyFull),
        _ => TlsMode::parse(&lower),
    };
    mode.ok_or_else(|| ExchangeError::Invalid("invalid client TLS mode".into()))
}
