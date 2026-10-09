// SPDX-License-Identifier: Apache-2.0
//! `settings.json` in the app-data dir (docs/15 §4): the user's preferences as
//! human-readable JSON. The webview owns the schema and validates every field;
//! this side only checks the envelope (an object with `"openrows": 1`) and
//! writes the file atomically, pretty-printed. The core itself reads the
//! session keepalive interval (`general.keepaliveSeconds`) and the result
//! store's memory budget (`results.memoryBudgetMb`).
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri deserialises command arguments into owned values"
)]
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use tauri::{Manager, State};

use crate::connections::{Connections, IpcError, IpcErrorKind};

/// File format version this build reads and writes.
const FORMAT: u64 = 1;
/// Largest settings file accepted; real files are a few kilobytes.
const MAX_BYTES: u64 = 1024 * 1024;
/// Session keepalive interval when the file does not set one (docs/14 §4).
const DEFAULT_KEEPALIVE_SECONDS: u64 = 30;

/// Where `settings.json` lives.
pub(crate) struct SettingsFile(PathBuf);

/// Manage the settings path next to the profiles and local store.
///
/// # Errors
/// The app-data dir cannot be resolved.
pub(crate) fn install(app: &mut tauri::App) -> Result<(), tauri::Error> {
    let path = app.path().app_data_dir()?.join("settings.json");
    app.manage(SettingsFile(path));
    Ok(())
}

/// Apply the saved keepalive interval; runs once [`Connections`] is managed.
/// An unreadable file keeps the defaults; the webview reports why.
pub(crate) fn apply_saved(app: &tauri::App) {
    if let Some(text) = text(app) {
        apply(&app.state::<Connections>(), &text);
    }
}

/// Apply the settings the core reads: the session keepalive interval.
fn apply(connections: &Connections, text: &str) {
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        connections.sessions.set_keepalive(keepalive(&value));
    }
}

/// `general.keepaliveSeconds`: `0` turns probing off, other values are
/// clamped to 5 s – 10 min; missing or invalid means 30 s (docs/14 §4).
fn keepalive(settings: &Value) -> Option<Duration> {
    let seconds = settings
        .pointer("/general/keepaliveSeconds")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_KEEPALIVE_SECONDS);
    (seconds > 0).then(|| Duration::from_secs(seconds.clamp(5, 600)))
}

/// The validated text of `settings.json` for readers in the core, e.g. the
/// result store's memory budget; `None` before the first save or when the
/// file cannot be used (the defaults apply then).
pub(crate) fn text(app: &tauri::App) -> Option<String> {
    read(&app.state::<SettingsFile>().0).ok().flatten()
}

fn storage(path: &Path, error: &impl std::fmt::Display) -> IpcError {
    IpcError::new(
        IpcErrorKind::Storage,
        format!("{}: {error}", path.display()),
    )
}

/// The parsed settings object, or why it cannot be used.
fn envelope(text: &str) -> Result<Value, IpcError> {
    let value: Value = serde_json::from_str(text)
        .map_err(|error| IpcError::invalid(format!("settings.json is not valid JSON: {error}")))?;
    let Some(object) = value.as_object() else {
        return Err(IpcError::invalid("settings.json must hold a JSON object"));
    };
    match object.get("openrows").and_then(Value::as_u64) {
        Some(FORMAT) => Ok(value),
        Some(_) => Err(IpcError::new(
            IpcErrorKind::Unsupported,
            "settings.json was written by a newer OpenRows",
        )),
        None => Err(IpcError::invalid(
            "settings.json has no \"openrows\": 1 field",
        )),
    }
}

/// Read the file; `None` when it does not exist yet.
fn read(path: &Path) -> Result<Option<String>, IpcError> {
    match fs::metadata(path) {
        Ok(meta) if meta.len() > MAX_BYTES => {
            return Err(storage(path, &"larger than 1 MiB"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(storage(path, &error)),
    }
    let text = fs::read_to_string(path).map_err(|error| storage(path, &error))?;
    envelope(&text)?;
    Ok(Some(text))
}

/// Validate and write pretty-printed JSON through a temp file and rename.
fn write(path: &Path, text: &str) -> Result<(), IpcError> {
    let value = envelope(text)?;
    let mut pretty = serde_json::to_string_pretty(&value)
        .map_err(|error| IpcError::new(IpcErrorKind::Internal, error.to_string()))?;
    pretty.push('\n');
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| storage(parent, &error))?;
    }
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, pretty).map_err(|error| storage(&temp, &error))?;
    fs::rename(&temp, path).map_err(|error| storage(path, &error))
}

/// The text of `settings.json`, or `null` before the first save.
#[tauri::command]
#[specta::specta]
pub(crate) fn settings_read(file: State<'_, SettingsFile>) -> Result<Option<String>, IpcError> {
    read(&file.0)
}

/// Replace `settings.json` with `text` (a JSON object with `"openrows": 1`).
#[tauri::command]
#[specta::specta]
pub(crate) fn settings_write(
    file: State<'_, SettingsFile>,
    connections: State<'_, Connections>,
    text: String,
) -> Result<(), IpcError> {
    write(&file.0, &text)?;
    apply(&connections, &text);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_reads_as_none() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        assert_eq!(read(&dir.path().join("settings.json"))?, None);
        Ok(())
    }

    #[test]
    fn writes_pretty_json_atomically_and_reads_it_back() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("nested/settings.json");
        write(&path, r#"{"openrows":1,"general":{"restoreSession":true}}"#)?;
        let text = read(&path)?.ok_or("file missing")?;
        assert!(text.contains("\n  \"general\": {\n"));
        assert!(!path.with_extension("json.tmp").exists());
        Ok(())
    }

    #[test]
    fn keepalive_defaults_clamps_and_turns_off() -> Result<(), serde_json::Error> {
        let interval = |text: &str| serde_json::from_str::<Value>(text).map(|v| keepalive(&v));
        let seconds = |value: u64| Some(Duration::from_secs(value));
        assert_eq!(interval(r#"{"openrows":1}"#)?, seconds(30));
        assert_eq!(
            interval(r#"{"general":{"keepaliveSeconds":60}}"#)?,
            seconds(60)
        );
        assert_eq!(
            interval(r#"{"general":{"keepaliveSeconds":1}}"#)?,
            seconds(5)
        );
        assert_eq!(interval(r#"{"general":{"keepaliveSeconds":0}}"#)?, None);
        assert_eq!(
            interval(r#"{"general":{"keepaliveSeconds":"x"}}"#)?,
            seconds(30)
        );
        Ok(())
    }

    #[test]
    fn rejects_bad_envelopes() {
        assert!(envelope("not json").is_err());
        assert!(envelope("[1]").is_err());
        assert!(envelope("{}").is_err());
        let newer = envelope(r#"{"openrows":2}"#).err().map(|error| error.kind);
        assert_eq!(newer, Some(IpcErrorKind::Unsupported));
        assert!(envelope(r#"{"openrows":1}"#).is_ok());
    }
}
