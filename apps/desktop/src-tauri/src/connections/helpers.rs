// SPDX-License-Identifier: Apache-2.0
//! Connect and exit integration for shared cloud helper processes.
use super::{Connections, IpcError, IpcErrorKind};
use openrows_net::helper::{Helper, HelperError, HelperSpec, tools};
use openrows_store::profile::ConnectionProfile;
use serde_json::Value;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
};
use tauri::Manager as _;

/// Validated tool path overrides, refreshed when settings.json changes.
#[derive(Default)]
pub(crate) struct HelperSettings(Mutex<HashMap<String, PathBuf>>);
impl HelperSettings {
    /// Read cloud tool overrides from the existing tools section.
    pub(crate) fn apply(&self, settings: &Value) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = tools::TOOLS
            .iter()
            .filter_map(|(key, tool)| {
                settings
                    .pointer(&format!("/tools/{key}"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|path| !path.is_empty())
                    .map(|path| ((*tool).into(), PathBuf::from(path)))
            })
            .collect();
    }
    fn path(&self, tool: &str) -> Option<PathBuf> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(tool)
            .cloned()
    }
}

/// Discover the required tool and acquire this profile's shared helper.
pub(crate) async fn acquire(
    state: &Connections,
    profile: &ConnectionProfile,
) -> Result<Arc<Helper>, IpcError> {
    let hop = profile
        .helper
        .as_ref()
        .ok_or_else(|| IpcError::invalid("No cloud route configured"))?;
    let path = std::env::var_os("PATH");
    let mut dirs = openrows_dump::search_dirs(path.as_deref());
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/bin"));
    }
    let override_path = state.helper_settings.path(hop.tool());
    let found = tools::discover(hop.tool(), override_path.as_deref(), &dirs)
        .await
        .map_err(|error| ipc_error(&error))?;
    let mut spec = HelperSpec::new(hop.clone(), found.path, found.version);
    if matches!(hop, openrows_net::helper::HelperHop::Ssm { .. }) {
        let plugin_path = state.helper_settings.path("session-manager-plugin");
        let plugin = tools::discover("session-manager-plugin", plugin_path.as_deref(), &dirs)
            .await
            .map_err(|error| ipc_error(&error))?;
        if let Some(parent) = plugin.path.parent() {
            dirs.insert(0, parent.to_path_buf());
        }
        let path =
            std::env::join_paths(dirs).map_err(|error| IpcError::invalid(error.to_string()))?;
        spec.environment
            .push(("PATH".into(), path.to_string_lossy().into_owned()));
    }
    state
        .helpers
        .acquire(&profile.id.to_string(), spec)
        .await
        .map_err(|error| ipc_error(&error))
}

/// Convert a typed helper failure at the IPC boundary.
pub(crate) fn ipc_error(error: &HelperError) -> IpcError {
    let kind = match error {
        HelperError::Invalid(_) => IpcErrorKind::Invalid,
        HelperError::NotFound { .. } => IpcErrorKind::NotFound,
        HelperError::Network { message, .. } if message.contains("Timed out") => {
            IpcErrorKind::Timeout
        }
        HelperError::Network { .. } => IpcErrorKind::Network,
    };
    IpcError::new(kind, error.to_string())
}

/// Tauri exit hook: synchronously wait for every helper process group to stop.
pub(crate) fn exit(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<Connections>() {
        tauri::async_runtime::block_on(state.helpers.shutdown());
    }
}
