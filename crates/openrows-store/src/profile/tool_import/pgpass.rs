// SPDX-License-Identifier: Apache-2.0
//! libpq colon and backslash escapes, comments and wildcard handling.
use crate::profile::{
    ConnectionProfile, EnvKind, EnvTag, ServerParams,
    exchange::{ExchangeError, ImportBatch, ImportEntry},
};
use openrows_secrets::SecretString;
use zeroize::Zeroizing;

/// Parse concrete pgpass rows; wildcard rows cannot identify a connection.
/// # Errors
/// Invalid concrete port or profile.
pub fn parse_pgpass(text: &str) -> Result<ImportBatch, ExchangeError> {
    let mut batch = ImportBatch::default();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = split(line);
        if fields.len() != 5 {
            batch.warnings.push(format!(
                "pgpass line {}: expected five fields; skipped",
                index + 1
            ));
            continue;
        }
        if fields[..4].iter().any(|field| field.as_str() == "*") {
            batch.warnings.push(format!(
                "pgpass line {}: wildcard entry skipped (no concrete connection)",
                index + 1
            ));
            continue;
        }
        let port = fields[1].parse().map_err(|_| {
            ExchangeError::Invalid(format!("pgpass line {}: invalid port", index + 1))
        })?;
        let params = ServerParams {
            host: fields[0].to_string(),
            port,
            database: Some(fields[2].to_string()),
            user: Some(fields[3].to_string()),
            ..ServerParams::default()
        };
        let mut profile = ConnectionProfile::new_postgres(
            &format!(
                "{}@{} / {}",
                fields[3].as_str(),
                fields[0].as_str(),
                fields[2].as_str()
            ),
            params,
            EnvTag::new(EnvKind::Local),
        );
        profile.imported = true;
        profile.validate()?;
        batch.entries.push(ImportEntry {
            profile,
            password: Some(SecretString::from(fields[4].as_str())),
        });
    }
    Ok(batch)
}
fn split(line: &str) -> Vec<Zeroizing<String>> {
    let mut fields = vec![Zeroizing::new(String::new())];
    let mut chars = line.chars();
    while let Some(character) = chars.next() {
        if character == ':' {
            fields.push(Zeroizing::new(String::new()));
        } else if let Some(field) = fields.last_mut() {
            if character == '\\' {
                field.push(chars.next().unwrap_or('\\'));
            } else {
                field.push(character);
            }
        }
    }
    fields
}
