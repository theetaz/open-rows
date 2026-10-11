// SPDX-License-Identifier: Apache-2.0
//! Notice edits made to `settings.json` outside the app (docs/15 §4) and
//! announce them with [`SettingsChanged`], so the webview reloads the file.
//! A once-a-second metadata check is enough for a hand-edited file and
//! needs no platform watcher; the text is only read when the size or
//! modification time moved, and the app's own writes are not announced.
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_specta::Event as _;
use tokio::time::{MissedTickBehavior, interval};

use crate::connections::Connections;
use crate::settings::{self, SettingsFile};

/// How often the file's metadata is checked.
const POLL: Duration = Duration::from_secs(1);

/// `settings.json` changed outside the app; the webview reads it again.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsChanged {}

impl tauri_specta::Event for SettingsChanged {
    const NAME: &'static str = "settings-changed";
}

/// Size and modification time: a cheap check before reading the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stamp {
    len: u64,
    modified: Option<SystemTime>,
}

fn stamp(path: &Path) -> Option<Stamp> {
    fs::metadata(path).ok().map(|meta| Stamp {
        len: meta.len(),
        modified: meta.modified().ok(),
    })
}

/// An edit made outside the app.
#[derive(Debug, PartialEq, Eq)]
enum Change {
    /// The file now holds this text.
    Edited(String),
    /// The file was removed.
    Removed,
}

/// Tracks the file between checks.
struct Watcher {
    last: Option<Stamp>,
}

impl Watcher {
    /// How the file changed since the last check, unless the app itself
    /// read or wrote what is there now.
    fn check(&mut self, file: &SettingsFile) -> Option<Change> {
        // Holding the known text keeps an app write from landing mid-check.
        let mut known = file.known();
        let now = stamp(&file.path);
        if now == self.last {
            return None;
        }
        self.last = now;
        let text = fs::read_to_string(&file.path).ok();
        if text == *known {
            return None;
        }
        known.clone_from(&text);
        Some(text.map_or(Change::Removed, Change::Edited))
    }
}

/// Check `settings.json` every second for the app's lifetime; apply the
/// keepalive interval of an external edit and tell the webview.
pub(crate) fn watch(app: &AppHandle) {
    let app = app.clone();
    let mut watcher = Watcher {
        last: stamp(&app.state::<SettingsFile>().path),
    };
    tauri::async_runtime::spawn(async move {
        let mut tick = interval(POLL);
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            let Some(change) = watcher.check(&app.state::<SettingsFile>()) else {
                continue;
            };
            app.state::<crate::ai::AiState>().revoke();
            if let Change::Edited(text) = change {
                apply_security(&app, &text);
                settings::apply(
                    &app.state::<Connections>(),
                    &app.state::<openrows_store::LocalStore>(),
                    &text,
                );
            }
            // Emitting only fails while the webview is gone.
            let _ = SettingsChanged {}.emit(&app);
        }
    });
}

fn apply_security(app: &AppHandle, text: &str) {
    if let Ok(value) = serde_json::from_str(text) {
        app.state::<crate::ai::AiState>().configure(&value);
        app.state::<crate::security::Security>()
            .configure(&value, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_at(path: &Path) -> SettingsFile {
        SettingsFile::new(path.to_path_buf())
    }

    #[test]
    fn reports_external_edits_but_not_the_apps_own_text() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("settings.json");
        let file = file_at(&path);
        let mut watcher = Watcher { last: stamp(&path) };
        assert_eq!(watcher.check(&file), None, "nothing there yet");
        fs::write(&path, r#"{"openrows":1}"#)?;
        assert_eq!(
            watcher.check(&file),
            Some(Change::Edited(r#"{"openrows":1}"#.to_owned()))
        );
        assert_eq!(watcher.check(&file), None, "already reported");
        let own = r#"{"openrows":1,"theme":"dark"}"#;
        *file.known() = Some(own.to_owned());
        fs::write(&path, own)?;
        assert_eq!(watcher.check(&file), None, "the app wrote this");
        fs::remove_file(&path)?;
        assert_eq!(watcher.check(&file), Some(Change::Removed));
        Ok(())
    }
}
