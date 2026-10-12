// SPDX-License-Identifier: Apache-2.0
//! Versioned secret-free exchange and preview-first conflict resolution (FR-CONN-12).
mod normalize;
mod preview;
use super::{ConnectionProfile, ProfileId, SecretRefs};
use openrows_secrets::SecretString;
pub use preview::{Conflict, ImportChoice, ImportDecision, apply_import, preview};
use serde_json::Value;

/// Schema shared by the importer, export tests and editors.
pub const CONNECTIONS_SCHEMA: &str =
    include_str!("../../../../apps/desktop/src-tauri/connections.schema.json");
/// An incoming profile with a separately held optional third-party password.
#[derive(Debug)]
pub struct ImportEntry {
    /// The non-secret profile, with a fresh id and imported provenance.
    pub profile: ConnectionProfile,
    /// Never serialized or sent to the webview; optional keychain import.
    pub password: Option<SecretString>,
}
/// A parsed preview, not yet persisted.
#[derive(Debug, Default)]
pub struct ImportBatch {
    /// Incoming profiles in source order.
    pub entries: Vec<ImportEntry>,
    /// Ignored fields, skipped entries and missing-file notes; never values.
    pub warnings: Vec<String>,
}
/// Typed exchange failures without input fragments.
#[derive(Debug, thiserror::Error)]
pub enum ExchangeError {
    /// JSON syntax or type error with location, never the offending value.
    #[error("Invalid connection JSON at line {line}, column {column}")]
    Json {
        /// One-based line.
        line: usize,
        /// One-based column.
        column: usize,
    },
    /// A newer version is not supported.
    #[error("Connection format v{0} is not supported; expected v1")]
    Version(u64),
    /// Envelope or profile validation failed.
    #[error("Invalid connections format: {0}")]
    Invalid(String),
    /// A profile could not be stored.
    #[error(transparent)]
    Profile(#[from] super::ProfileError),
}
fn json_error(error: &serde_json::Error) -> ExchangeError {
    ExchangeError::Json {
        line: error.line(),
        column: error.column(),
    }
}
/// Serialize selected profiles without ids, secret references or opaque SQL/options.
/// # Errors
/// The profile cannot be represented by the documented v1 schema.
pub fn export(profiles: &[ConnectionProfile]) -> Result<String, ExchangeError> {
    let profiles = profiles
        .iter()
        .map(portable)
        .collect::<Result<Vec<_>, _>>()?;
    let document =
        serde_json::json!({"format":"openrows.connections", "version":1, "profiles":profiles});
    normalize::validate(&document)?;
    serde_json::to_string_pretty(&document).map_err(|error| json_error(&error))
}
fn portable(profile: &ConnectionProfile) -> Result<Value, ExchangeError> {
    let mut value = serde_json::to_value(profile).map_err(|error| json_error(&error))?;
    if let Some(fields) = value.as_object_mut() {
        for field in ["id", "openrows", "secrets", "imported", "startupCommands"] {
            fields.remove(field);
        }
    }
    // Opaque driver options may hold secrets. Only the documented safe option is portable.
    if let Some(options) = value
        .pointer_mut("/params/options")
        .and_then(Value::as_object_mut)
    {
        options.retain(|key, _| key == "connect_timeout");
    }
    Ok(value)
}
/// Parse and validate an open export, collecting unknown-field warnings.
/// # Errors
/// Syntax location, unsupported version or invalid profile settings.
pub fn parse(text: &str) -> Result<ImportBatch, ExchangeError> {
    let value = serde_json::from_str(text).map_err(|error| json_error(&error))?;
    parse_value(value)
}
/// Parse shared definitions, refusing fields outside the secret-free v1 schema.
/// Safety-policy warnings remain available to the normal import preview.
/// # Errors
/// Syntax location, unsupported fields or invalid profile settings.
pub fn parse_strict(text: &str) -> Result<ImportBatch, ExchangeError> {
    let value = serde_json::from_str(text).map_err(|error| json_error(&error))?;
    normalize::validate(&value)?;
    parse_value(value)
}
fn parse_value(mut value: Value) -> Result<ImportBatch, ExchangeError> {
    if let Some(version) = value.get("version").and_then(Value::as_u64)
        && version != 1
    {
        return Err(ExchangeError::Version(version));
    }
    let mut batch = ImportBatch::default();
    normalize::unknown_fields(&mut value, &mut batch.warnings)?;
    normalize::validate(&value)?;
    let profiles = value["profiles"]
        .as_array()
        .ok_or_else(|| ExchangeError::Invalid("profiles must be an array".into()))?;
    for value in profiles {
        let mut value = value.clone();
        value["openrows"] = Value::from(1);
        value["id"] = serde_json::to_value(ProfileId::new()).map_err(|error| json_error(&error))?;
        let mut profile: ConnectionProfile =
            serde_json::from_value(value).map_err(|error| json_error(&error))?;
        profile.secrets = SecretRefs::default();
        profile.imported = true;
        if profile.env_tag.kind == super::EnvKind::Prod
            && profile.safety == super::Safety::Unrestricted
        {
            profile.safety = super::Safety::ConfirmWrites;
            batch
                .warnings
                .push("Production import upgraded unrestricted policy to ConfirmWrites".into());
        }
        profile.validate()?;
        batch.entries.push(ImportEntry {
            profile,
            password: None,
        });
    }
    Ok(batch)
}
/// Compare only portable fields, disregarding ids and local secret references.
/// # Errors
/// Serialization failed.
pub fn equivalent(
    left: &ConnectionProfile,
    right: &ConnectionProfile,
) -> Result<bool, ExchangeError> {
    Ok(portable(left)? == portable(right)?)
}
