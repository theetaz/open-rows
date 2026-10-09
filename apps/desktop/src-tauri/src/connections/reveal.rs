// SPDX-License-Identifier: Apache-2.0
//! Explicit reveal of one saved secret (NFR-SEC-01, docs/12 §3): the only
//! path on which a stored password or passphrase reaches the webview. It is
//! read from the keychain on demand, for one profile and one field, and never
//! becomes part of a profile DTO or a log line.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri deserialises command arguments into owned values"
)]
use openrows_store::profile::{ProfileService, SecretSlot};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

use super::Connections;
use super::error::{IpcError, IpcErrorKind};
use super::profiles::profile_id;

/// Which saved secret of a connection to reveal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SecretFieldDto {
    Password,
    SshPassword,
    SshPassphrase,
    TlsKeyPassphrase,
    ProxyPassword,
}

impl From<SecretFieldDto> for SecretSlot {
    fn from(field: SecretFieldDto) -> Self {
        match field {
            SecretFieldDto::Password => Self::Password,
            SecretFieldDto::SshPassword => Self::SshPassword,
            SecretFieldDto::SshPassphrase => Self::SshPassphrase,
            SecretFieldDto::TlsKeyPassphrase => Self::TlsKeyPassphrase,
            SecretFieldDto::ProxyPassword => Self::ProxyPassword,
        }
    }
}

/// A secret the user asked to see. `Debug` redacts the value.
#[derive(Clone, Serialize, Type)]
pub(crate) struct RevealedSecret {
    value: String,
}

impl std::fmt::Debug for RevealedSecret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RevealedSecret([redacted])")
    }
}

fn reveal(
    service: &ProfileService,
    id: &str,
    field: SecretFieldDto,
) -> Result<RevealedSecret, IpcError> {
    let profile = service.get(profile_id(id)?)?;
    let secret = service
        .secret(&profile, field.into())?
        .ok_or_else(|| IpcError::new(IpcErrorKind::NotFound, "Nothing is saved in the Keychain"))?;
    Ok(RevealedSecret {
        value: secret.expose().to_owned(),
    })
}

/// Read one saved secret of a connection from the keychain because the user
/// clicked Reveal; nothing else ever returns a stored secret.
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn profile_reveal_secret(
    state: State<'_, Connections>,
    id: String,
    field: SecretFieldDto,
) -> Result<RevealedSecret, IpcError> {
    reveal(&state.profiles, &id, field)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use openrows_secrets::MemorySecretStore;
    use openrows_store::profile::ProfileFiles;

    use super::super::dto::{EnvKindDto, PasswordInput, ProfileDto, tests::input};
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn reveal_reads_one_secret_on_demand_and_dtos_never_carry_it() -> TestResult {
        let dir = tempfile::tempdir()?;
        let service = ProfileService::new(
            ProfileFiles::new(dir.path()),
            Arc::new(MemorySecretStore::new()),
        );
        let mut form = input(EnvKindDto::Dev);
        form.password = PasswordInput::Set {
            value: "pw-reveal-synthetic".to_owned(),
        };
        let mut profile = form.new_profile();
        form.apply(&mut profile)?;
        let created = service.create_with(profile, &form.secret_changes()?)?;
        let id = created.id.to_string();
        let revealed = reveal(&service, &id, SecretFieldDto::Password)?;
        assert_eq!(revealed.value, "pw-reveal-synthetic");
        assert!(!format!("{revealed:?}").contains("pw-reveal"));
        let dto = serde_json::to_string(&ProfileDto::from(&service.get(created.id)?))?;
        assert!(!dto.contains("pw-reveal"), "{dto}");
        let missing = reveal(&service, &id, SecretFieldDto::SshPassword);
        assert!(missing.is_err_and(|error| error.kind == IpcErrorKind::NotFound));
        Ok(())
    }
}
