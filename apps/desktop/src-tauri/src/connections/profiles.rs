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
    ConnectionProfile, EnvKind, EnvTag, PasswordChange, ProfileId, ProfileService,
    parse_postgres_url,
};
use tauri::State;

use super::Connections;
use super::dto::{PasswordInput, ProfileDto, ProfileInput, ProfileList, UrlDraft};
use super::error::IpcError;

pub(crate) fn profile_id(id: &str) -> Result<ProfileId, IpcError> {
    ProfileId::from_str(id).map_err(IpcError::from)
}

/// The password `input` asks for; `stored` is the profile being edited.
pub(crate) fn resolve_password(
    service: &ProfileService,
    stored: Option<&ConnectionProfile>,
    input: &PasswordInput,
) -> Result<Option<SecretString>, IpcError> {
    match input {
        PasswordInput::Keep => match stored {
            Some(profile) => Ok(service.password(profile)?),
            None => Ok(None),
        },
        PasswordInput::Clear => Ok(None),
        PasswordInput::Set { value } => Ok(Some(SecretString::from(value.as_str()))),
        PasswordInput::FromUrl { url } => Ok(parse_postgres_url(url)?.password),
    }
}

fn password_change(input: &PasswordInput) -> Result<PasswordChange, IpcError> {
    Ok(match input {
        PasswordInput::Keep => PasswordChange::Keep,
        PasswordInput::Clear => PasswordChange::Clear,
        PasswordInput::Set { value } => PasswordChange::Set(SecretString::from(value.as_str())),
        PasswordInput::FromUrl { url } => parse_postgres_url(url)?
            .password
            .map_or(PasswordChange::Keep, PasswordChange::Set),
    })
}

fn list(service: &ProfileService) -> Result<ProfileList, IpcError> {
    let loaded = service.list()?;
    Ok(ProfileList {
        profiles: loaded.profiles.iter().map(ProfileDto::from).collect(),
        unreadable: loaded.unreadable.iter().map(ToString::to_string).collect(),
    })
}

fn create(service: &ProfileService, input: &ProfileInput) -> Result<ProfileDto, IpcError> {
    let mut profile =
        ConnectionProfile::new_postgres(&input.name, input.params(), EnvTag::new(EnvKind::Local));
    input.apply(&mut profile)?;
    let password = resolve_password(service, None, &input.password)?;
    Ok(ProfileDto::from(
        &service.create(profile, password.as_ref())?,
    ))
}

fn update(
    service: &ProfileService,
    id: &str,
    input: &ProfileInput,
) -> Result<ProfileDto, IpcError> {
    let mut profile = service.get(profile_id(id)?)?;
    input.apply(&mut profile)?;
    let change = password_change(&input.password)?;
    Ok(ProfileDto::from(&service.update(profile, change)?))
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

/// Save a new connection; its password goes to the keychain.
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn profile_create(
    state: State<'_, Connections>,
    input: ProfileInput,
) -> Result<ProfileDto, IpcError> {
    create(&state.profiles, &input)
}

/// Replace a saved connection's fields and apply the password change.
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn profile_update(
    state: State<'_, Connections>,
    id: String,
    input: ProfileInput,
) -> Result<ProfileDto, IpcError> {
    update(&state.profiles, &id, &input)
}

/// Copy a connection (and its password) as "<name> copy".
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

/// Delete a connection and its stored password.
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn profile_delete(state: State<'_, Connections>, id: String) -> Result<(), IpcError> {
    Ok(state.profiles.delete(profile_id(&id)?)?)
}

/// Form fields from a pasted `postgres://` URL, without the password itself.
#[tauri::command]
#[specta::specta]
pub(crate) fn profile_parse_url(url: String) -> Result<UrlDraft, IpcError> {
    Ok(parse_postgres_url(&url)?.into())
}

#[cfg(test)]
mod tests {
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

    fn created_id(json: &str) -> Result<String, Box<dyn std::error::Error>> {
        let value: serde_json::Value = serde_json::from_str(json)?;
        Ok(value["id"].as_str().ok_or("missing id")?.to_owned())
    }
}
