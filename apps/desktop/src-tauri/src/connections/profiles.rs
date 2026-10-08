// SPDX-License-Identifier: Apache-2.0
//! Profile commands: list, get, create, update, duplicate, delete, search and
//! URL parsing. Logic takes a [`ProfileService`] so tests use an in-memory
//! secret store; the Tauri commands only unwrap state.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri deserialises command arguments into owned values"
)]
use std::str::FromStr;

use openrows_secrets::SecretString;
use openrows_store::profile::{
    ConnectionProfile, PasswordChange, ProfileId, ProfileService, SecretSlot, parse_url,
};
use tauri::State;

use super::Connections;
use super::dto::{PasswordInput, ProfileDto, ProfileInput, ProfileList, UrlDraft, password_change};
use super::error::IpcError;

pub(crate) fn profile_id(id: &str) -> Result<ProfileId, IpcError> {
    ProfileId::from_str(id).map_err(IpcError::from)
}

/// The secret in `slot` that `change` asks for; `stored` is the profile
/// being edited. Keeping reads the keychain only when a secret is stored.
pub(crate) fn resolve_secret(
    service: &ProfileService,
    stored: Option<&ConnectionProfile>,
    slot: SecretSlot,
    change: PasswordChange,
) -> Result<Option<SecretString>, IpcError> {
    match change {
        PasswordChange::Keep => match stored {
            Some(profile) => Ok(service.secret(profile, slot)?),
            None => Ok(None),
        },
        PasswordChange::Clear => Ok(None),
        PasswordChange::Set(value) => Ok(Some(value)),
    }
}

/// The password `input` asks for; `stored` is the profile being edited.
pub(crate) fn resolve_password(
    service: &ProfileService,
    stored: Option<&ConnectionProfile>,
    input: &PasswordInput,
) -> Result<Option<SecretString>, IpcError> {
    if let PasswordInput::FromUrl { url } = input {
        // A URL without a password means no password, not the stored one.
        return Ok(parse_url(url)?.password);
    }
    resolve_secret(
        service,
        stored,
        SecretSlot::Password,
        password_change(input)?,
    )
}

fn list(service: &ProfileService) -> Result<ProfileList, IpcError> {
    let loaded = service.list()?;
    Ok(ProfileList {
        profiles: loaded.profiles.iter().map(ProfileDto::from).collect(),
        unreadable: loaded.unreadable.iter().map(ToString::to_string).collect(),
    })
}

fn create(service: &ProfileService, input: &ProfileInput) -> Result<ProfileDto, IpcError> {
    let mut profile = input.new_profile();
    input.apply(&mut profile)?;
    super::drivers::driver(profile.driver)?;
    let changes = input.secret_changes()?;
    Ok(ProfileDto::from(&service.create_with(profile, &changes)?))
}

fn update(
    service: &ProfileService,
    id: &str,
    input: &ProfileInput,
) -> Result<ProfileDto, IpcError> {
    let mut profile = service.get(profile_id(id)?)?;
    input.apply(&mut profile)?;
    let changes = input.secret_changes()?;
    Ok(ProfileDto::from(&service.update_with(profile, &changes)?))
}

fn search(service: &ProfileService, query: &str) -> Result<Vec<ProfileDto>, IpcError> {
    Ok(service
        .search(query)?
        .iter()
        .map(ProfileDto::from)
        .collect())
}

/// Saved connections, sorted by group then name.
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn profiles_list(state: State<'_, Connections>) -> Result<ProfileList, IpcError> {
    list(&state.profiles)
}

/// Saved connections matching every whitespace-separated term.
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn profiles_search(
    state: State<'_, Connections>,
    query: String,
) -> Result<Vec<ProfileDto>, IpcError> {
    search(&state.profiles, &query)
}

/// One saved connection.
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn profile_get(
    state: State<'_, Connections>,
    id: String,
) -> Result<ProfileDto, IpcError> {
    Ok(ProfileDto::from(&state.profiles.get(profile_id(&id)?)?))
}

/// Save a new connection; its password and passphrases go to the keychain.
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn profile_create(
    state: State<'_, Connections>,
    input: ProfileInput,
) -> Result<ProfileDto, IpcError> {
    create(&state.profiles, &input)
}

/// Replace a saved connection's fields and apply the secret changes.
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn profile_update(
    state: State<'_, Connections>,
    id: String,
    input: ProfileInput,
) -> Result<ProfileDto, IpcError> {
    let updated = update(&state.profiles, &id, &input)?;
    // New sessions use the edited route; open ones keep their tunnel.
    state.tunnels.forget(&id);
    Ok(updated)
}

/// Copy a connection (and its secrets) as "<name> copy".
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn profile_duplicate(
    state: State<'_, Connections>,
    id: String,
) -> Result<ProfileDto, IpcError> {
    Ok(ProfileDto::from(
        &state.profiles.duplicate(profile_id(&id)?)?,
    ))
}

/// Delete a connection and its stored secrets.
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn profile_delete(state: State<'_, Connections>, id: String) -> Result<(), IpcError> {
    state.profiles.delete(profile_id(&id)?)?;
    state.tunnels.forget(&id);
    Ok(())
}

/// Form fields from a pasted connection URL, without the password itself.
#[tauri::command]
#[specta::specta]
pub(crate) fn profile_parse_url(url: String) -> Result<UrlDraft, IpcError> {
    Ok(parse_url(&url)?.into())
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Arc;

    use openrows_secrets::MemorySecretStore;
    use openrows_store::profile::ProfileFiles;

    use super::super::dto::EnvKindDto;
    use super::super::dto::tests::input;
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn service(dir: &tempfile::TempDir) -> ProfileService {
        ProfileService::new(
            ProfileFiles::new(dir.path()),
            Arc::new(MemorySecretStore::new()),
        )
    }

    fn files_text(dir: &tempfile::TempDir) -> Result<String, std::io::Error> {
        let mut text = String::new();
        for entry in std::fs::read_dir(dir.path())? {
            text.push_str(&std::fs::read_to_string(entry?.path())?);
        }
        Ok(text)
    }

    #[test]
    fn create_from_url_stores_the_password_outside_the_profile() -> TestResult {
        let dir = tempfile::tempdir()?;
        let service = service(&dir);
        let mut form = input(EnvKindDto::Dev);
        form.password = PasswordInput::FromUrl {
            url: "postgres://app:pw-synthetic@localhost/orders".to_owned(),
        };
        let created = create(&service, &form)?;
        let json = serde_json::to_string(&created)?;
        assert!(json.contains("\"hasPassword\":true"));
        assert!(!json.contains("pw-synthetic"));
        assert!(!files_text(&dir)?.contains("pw-synthetic"));
        let stored = service.get(profile_id(&created_id(&json)?)?)?;
        let password = resolve_password(&service, Some(&stored), &PasswordInput::Keep)?;
        assert_eq!(
            password.as_ref().map(SecretString::expose),
            Some("pw-synthetic")
        );
        Ok(())
    }

    #[test]
    fn update_keeps_or_clears_the_password_and_lists_search() -> TestResult {
        let dir = tempfile::tempdir()?;
        let service = service(&dir);
        let mut form = input(EnvKindDto::Prod);
        form.password = PasswordInput::Set {
            value: "pw-synthetic".to_owned(),
        };
        let id = created_id(&serde_json::to_string(&create(&service, &form)?)?)?;
        form.name = "Renamed".to_owned();
        form.password = PasswordInput::Keep;
        let kept = serde_json::to_value(update(&service, &id, &form)?)?;
        assert_eq!(
            (kept["name"].as_str(), kept["hasPassword"].as_bool()),
            (Some("Renamed"), Some(true))
        );
        form.password = PasswordInput::Clear;
        let cleared = serde_json::to_value(update(&service, &id, &form)?)?;
        assert_eq!(cleared["hasPassword"], false);
        assert_eq!(list(&service)?.profiles.len(), 1);
        assert_eq!(search(&service, "renamed")?.len(), 1);
        assert!(search(&service, "missing")?.is_empty());
        Ok(())
    }

    #[test]
    fn unknown_ids_are_typed_errors() -> TestResult {
        let dir = tempfile::tempdir()?;
        let service = service(&dir);
        let bad = update(&service, "not-a-uuid", &input(EnvKindDto::Local));
        assert!(bad.is_err_and(|error| error.kind == super::super::error::IpcErrorKind::Invalid));
        let missing = update(
            &service,
            &ProfileId::new().to_string(),
            &input(EnvKindDto::Local),
        );
        assert!(
            missing.is_err_and(|error| error.kind == super::super::error::IpcErrorKind::NotFound)
        );
        Ok(())
    }

    /// Save the test form without a password; returns the new profile id.
    pub(crate) fn create_passwordless(
        service: &ProfileService,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let created = create(service, &input(EnvKindDto::Local))?;
        created_id(&serde_json::to_string(&created)?)
    }

    fn created_id(json: &str) -> Result<String, Box<dyn std::error::Error>> {
        let value: serde_json::Value = serde_json::from_str(json)?;
        Ok(value["id"].as_str().ok_or("missing id")?.to_owned())
    }
}
