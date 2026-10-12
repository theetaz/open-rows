// SPDX-License-Identifier: Apache-2.0
//! Release-owned channel selection; signature verification stays in Tauri's updater.
use std::time::Duration;

use serde::Serialize;
use tauri::Manager;
use tauri_plugin_updater::UpdaterExt;

use crate::connections::{IpcError, IpcErrorKind};

/// Metadata for a Tauri updater resource owned by this webview.
#[derive(Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateReply {
    rid: u32,
    current_version: String,
    version: String,
    body: Option<String>,
}

/// Check a compiled-in channel, never a webview-supplied URL or public key.
#[tauri::command]
#[specta::specta]
pub(crate) async fn update_check(
    webview: tauri::Webview,
    channel: String,
) -> Result<Option<UpdateReply>, IpcError> {
    let config = &webview.config().plugins.0;
    let endpoint = config
        .get("openrowsUpdates")
        .and_then(|feeds| feeds.get(&channel))
        .and_then(serde_json::Value::as_str);
    let url = channel_url(&channel, endpoint)?;
    let update = webview
        .updater_builder()
        .endpoints(vec![url])
        .map_err(|error| update_error(&error))?
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| update_error(&error))?
        .check()
        .await
        .map_err(|error| update_error(&error))?;
    Ok(update.map(|update| update_reply(&webview, update)))
}

fn update_reply(webview: &tauri::Webview, update: tauri_plugin_updater::Update) -> UpdateReply {
    let current_version = update.current_version.clone();
    let version = update.version.clone();
    let body = update.body.clone();
    let rid = webview.resources_table().add(update);
    UpdateReply {
        rid,
        current_version,
        version,
        body,
    }
}

fn channel_url(channel: &str, endpoint: Option<&str>) -> Result<tauri::Url, IpcError> {
    if !matches!(channel, "stable" | "beta") {
        return Err(IpcError::new(
            IpcErrorKind::Unsupported,
            "Unknown update channel",
        ));
    }
    let url = endpoint
        .and_then(|value| tauri::Url::parse(value).ok())
        .filter(|url| {
            url.scheme() == "https" && url.username().is_empty() && url.password().is_none()
        });
    url.ok_or_else(|| {
        IpcError::new(
            IpcErrorKind::Unsupported,
            "No signed update feed configured",
        )
    })
}

fn update_error(error: &tauri_plugin_updater::Error) -> IpcError {
    IpcError::new(IpcErrorKind::Internal, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::channel_url;

    #[test]
    fn channel_selection_fails_closed() {
        for (channel, endpoint) in [
            ("stable", None),
            ("nightly", Some("https://example.invalid")),
            ("beta", Some("http://example.invalid")),
            ("stable", Some("https://user:password@example.invalid")),
        ] {
            assert!(channel_url(channel, endpoint).is_err());
        }
        assert!(channel_url("beta", Some("https://example.invalid/beta.json")).is_ok());
    }
}
