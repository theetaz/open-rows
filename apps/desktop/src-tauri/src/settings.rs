// SPDX-License-Identifier: Apache-2.0
//! `settings.json` in the app-data dir (docs/15 §4): the user's preferences as
//! human-readable JSON. The webview owns the schema and validates every field;
//! this side only checks the envelope (an object with `"openrows": 1`) and
//! writes the file atomically, pretty-printed, with a `$schema` reference to
//! the JSON Schema written beside it. The core itself reads the session
//! keepalive interval (`general.keepaliveSeconds`) and the result store's
//! memory budget (`results.memoryBudgetMb`). Edits made outside the app are
//! noticed by [`watch`](crate::settings_watch::watch), which applies them
//! and announces [`SettingsChanged`](crate::settings_watch::SettingsChanged).
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri deserialises command arguments into owned values"
)]
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use openrows_store::{HistoryRetention, LocalStore};
use serde_json::{Map, Value};
use tauri::{Manager, State};

use crate::connections::{Connections, IpcError, IpcErrorKind};

/// File format version this build reads and writes.
const FORMAT: u64 = 1;
/// Largest settings file accepted; real files are a few kilobytes.
const MAX_BYTES: u64 = 1024 * 1024;
/// Session keepalive interval when the file does not set one (docs/14 §4).
const DEFAULT_KEEPALIVE_SECONDS: u64 = 30;

/// The JSON Schema of `settings.json`, written beside it for editors.
const SCHEMA: &str = include_str!("../settings.schema.json");
/// File name of the schema, and the `$schema` reference to it.
const SCHEMA_NAME: &str = "settings.schema.json";

/// Where `settings.json` lives, and the text the app last read or wrote, so
/// the watcher can tell edits made outside the app from its own writes.
pub(crate) struct SettingsFile {
    pub(crate) path: PathBuf,
    known: Mutex<Option<String>>,
}

impl SettingsFile {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self {
            path,
            known: Mutex::new(None),
        }
    }

    /// The last text read or written; holding it orders writes and checks.
    pub(crate) fn known(&self) -> MutexGuard<'_, Option<String>> {
        self.known.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Manage the settings path next to the profiles and local store, and write
/// the schema beside it.
///
pub(crate) fn install(app: &mut tauri::App) {
    let path = app
        .state::<crate::locations::Locations>()
        .data
        .clone()
        .join("settings.json");
    if let Err(error) = write_schema(&path) {
        tracing::warn!(error = %error.message, "settings schema not written");
    }
    let file = SettingsFile::new(path);
    *file.known() = read(&file.path).ok().flatten();
    app.manage(file);
}

/// Write the JSON Schema next to `settings` unless it is already current.
fn write_schema(settings: &Path) -> Result<(), IpcError> {
    let path = settings.with_file_name(SCHEMA_NAME);
    if fs::read_to_string(&path).is_ok_and(|text| text == SCHEMA) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| storage(parent, &error))?;
    }
    fs::write(&path, SCHEMA).map_err(|error| storage(&path, &error))
}

/// Apply the saved keepalive interval; runs once [`Connections`] is managed.
/// An unreadable file keeps the defaults; the webview reports why.
pub(crate) fn apply_saved(app: &tauri::App) {
    if let Some(text) = text(app) {
        if let Ok(value) = serde_json::from_str(&text) {
            app.state::<crate::ai::AiState>().configure(&value);
        }
        apply(
            &app.state::<Connections>(),
            &app.state::<LocalStore>(),
            &text,
        );
    } else {
        let _ = app
            .state::<LocalStore>()
            .set_monitor_profiles(&[], crate::queries::history::now_ms());
        apply_retention(&app.state::<LocalStore>(), &Value::Null);
    }
}

/// Apply the settings the core reads at run time: the session keepalive
/// interval and the default statement timeout (also after an edit made
/// outside the app).
pub(crate) fn apply(connections: &Connections, local: &LocalStore, text: &str) {
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        connections
            .app
            .state::<crate::mcp_client::State>()
            .invalidate();
        crate::mcp::configure(&connections.app);
        connections.sessions.set_keepalive(keepalive(&value));
        connections.limits.apply(&value);
        connections.helper_settings.apply(&value);
        apply_retention(local, &value);
        let profiles = value
            .pointer("/monitoring/enabledProfiles")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(crate::connections::monitor::profile_key_for_history)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if let Err(error) = local.set_monitor_profiles(&profiles, crate::queries::history::now_ms())
        {
            tracing::warn!(%error, "monitor history settings not applied");
        }
    }
}

/// Apply count and age settings before new history writes.
fn apply_retention(local: &LocalStore, value: &Value) {
    let policy = value
        .get("history")
        .and_then(|value| serde_json::from_value::<HistoryRetention>(value.clone()).ok())
        .unwrap_or_default();
    if let Err(error) = local.set_history_retention(policy, crate::queries::history::now_ms()) {
        tracing::warn!(%error, "history retention not applied");
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
    read(&app.state::<SettingsFile>().path).ok().flatten()
}

/// The current `settings.json` as JSON for readers in the core at run time
/// (e.g. the tool paths of backups); `None` before the first save or when
/// the file cannot be used.
pub(crate) fn current(file: &SettingsFile) -> Option<Value> {
    let text = read(&file.path).ok().flatten()?;
    serde_json::from_str(&text).ok()
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
    crate::ai::validate_settings(&value)?;
    crate::mcp::parse(&value)?;
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

/// The settings object with `$schema` first, pointing at the schema file.
fn with_schema(value: Value) -> Value {
    let Value::Object(fields) = value else {
        return value;
    };
    let mut object = Map::new();
    object.insert(
        "$schema".to_owned(),
        Value::from(format!("./{SCHEMA_NAME}")),
    );
    object.extend(fields.into_iter().filter(|(key, _)| key != "$schema"));
    Value::Object(object)
}

/// Validate and write pretty-printed JSON through a temp file and rename;
/// returns the text written.
fn write(path: &Path, text: &str) -> Result<String, IpcError> {
    let value = with_schema(envelope(text)?);
    let mut pretty = serde_json::to_string_pretty(&value)
        .map_err(|error| IpcError::new(IpcErrorKind::Internal, error.to_string()))?;
    pretty.push('\n');
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| storage(parent, &error))?;
    }
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, &pretty).map_err(|error| storage(&temp, &error))?;
    fs::rename(&temp, path).map_err(|error| storage(path, &error))?;
    Ok(pretty)
}

/// The text of `settings.json`, or `null` before the first save.
#[tauri::command]
#[specta::specta]
pub(crate) fn settings_read(file: State<'_, SettingsFile>) -> Result<Option<String>, IpcError> {
    let mut known = file.known();
    let text = read(&file.path)?;
    known.clone_from(&text);
    Ok(text)
}

/// Replace `settings.json` with `text` (a JSON object with `"openrows": 1`).
#[tauri::command]
#[specta::specta]
pub(crate) fn settings_write(
    app: tauri::AppHandle,
    file: State<'_, SettingsFile>,
    connections: State<'_, Connections>,
    local: State<'_, LocalStore>,
    text: String,
) -> Result<(), IpcError> {
    let mut known = file.known();
    *known = Some(write(&file.path, &text)?);
    if let Ok(value) = serde_json::from_str(&text) {
        app.state::<crate::ai::AiState>().configure(&value);
    }
    apply(&connections, &local, &text);
    if let Ok(value) = serde_json::from_str(&text) {
        app.state::<crate::security::Security>()
            .configure(&value, false);
    }
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
        assert!(text.starts_with("{\n  \"$schema\": \"./settings.schema.json\",\n"));
        assert!(!path.with_extension("json.tmp").exists());
        Ok(())
    }

    #[test]
    fn schema_is_valid_json_written_once_beside_the_settings()
    -> Result<(), Box<dyn std::error::Error>> {
        let schema: Value = serde_json::from_str(SCHEMA)?;
        assert_eq!(schema["properties"]["openrows"]["const"], 1);
        let budget = &schema["properties"]["results"]["properties"]["memoryBudgetMb"];
        assert_eq!(
            (budget["minimum"].as_u64(), budget["maximum"].as_u64()),
            (Some(64), Some(65536))
        );
        let dir = tempfile::tempdir()?;
        let settings = dir.path().join("settings.json");
        write_schema(&settings)?;
        let path = dir.path().join(SCHEMA_NAME);
        assert_eq!(fs::read_to_string(&path)?, SCHEMA);
        fs::write(&path, "{}")?;
        write_schema(&settings)?;
        assert_eq!(fs::read_to_string(&path)?, SCHEMA);
        Ok(())
    }

    #[test]
    fn an_existing_schema_reference_is_replaced_not_duplicated()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("settings.json");
        let text = write(&path, r#"{"$schema":"x","openrows":1}"#)?;
        assert_eq!(text.matches("$schema").count(), 1);
        assert!(text.contains("./settings.schema.json"));
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
