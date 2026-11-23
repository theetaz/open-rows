// SPDX-License-Identifier: Apache-2.0
//! Connect the user's installed agent CLIs (Claude Code, Codex, Gemini CLI)
//! to the built-in server through each tool's own `mcp add/get/remove`
//! commands. Nothing here edits another tool's configuration files or reads
//! its credentials; detection only finds executables and asks `--version`.
mod commands;
mod detect;
mod plan;
mod probe;
mod run;
mod verify;

pub use commands::{BridgeTarget, add_args, check_args, display, remove_args};
pub use detect::{AppInstall, detect, locate, search_directories};
pub use plan::{APP_TOKEN_SECONDS, PREFERRED_PORT, client_matches, new_client_id, stable_port};
pub use probe::{forget_token, store_token, test_bridge};
pub use run::{Output, run};
pub use verify::{Registration, parse_check};

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Name of the server entry in every agent app's MCP configuration.
pub const SERVER_NAME: &str = "openrows";

/// An agent CLI that can use `OpenRows` as an MCP server.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum AgentApp {
    /// Anthropic's Claude Code CLI.
    Claude,
    /// The Codex CLI.
    Codex,
    /// Google's Gemini CLI.
    Gemini,
}
impl AgentApp {
    /// Every supported app, in display order.
    pub const ALL: [Self; 3] = [Self::Claude, Self::Codex, Self::Gemini];
    /// Stable identifier used in links and activity.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
        }
    }
    /// Parse a stored identifier.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|app| app.id() == id)
    }
}

/// Failure of an Apps operation; messages are safe to show.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AppError {
    /// The CLI could not be found or started.
    #[error("The app's command-line tool was not found")]
    NotInstalled,
    /// The `openrows-mcp` bridge is missing beside the application.
    #[error("The openrows-mcp bridge was not found beside OpenRows")]
    BridgeMissing,
    /// The application runs from a temporary translocated copy.
    #[error("Move OpenRows to the Applications folder before connecting apps")]
    Translocated,
    /// The command did not finish in time.
    #[error("The command did not finish in time")]
    Timeout,
    /// The command failed; carries the tool's last message line.
    #[error("The command failed: {0}")]
    Command(String),
    /// The tool's output could not be understood.
    #[error("The app's response was not recognised")]
    Unrecognised,
    /// The keychain or the bridge's credential step failed.
    #[error("The app's credential could not be saved in the keychain")]
    Keychain,
    /// The app could not be reached through the bridge.
    #[error("The bridge could not reach OpenRows: {0}")]
    Bridge(String),
}

/// The bridge executable installed beside the running application.
/// # Errors
/// Missing bridge, or an application running from a translocated copy.
pub fn bridge_path(app_executable: &Path) -> Result<PathBuf, AppError> {
    if app_executable
        .components()
        .any(|part| part.as_os_str() == "AppTranslocation")
    {
        return Err(AppError::Translocated);
    }
    let name = if cfg!(windows) {
        "openrows-mcp.exe"
    } else {
        "openrows-mcp"
    };
    let bridge = app_executable
        .parent()
        .ok_or(AppError::BridgeMissing)?
        .join(name);
    detect::executable(&bridge)
        .then_some(bridge)
        .ok_or(AppError::BridgeMissing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_round_trip() {
        for app in AgentApp::ALL {
            assert_eq!(AgentApp::from_id(app.id()), Some(app));
        }
        assert_eq!(AgentApp::from_id("cursor"), None);
    }

    #[test]
    fn bridge_must_sit_beside_an_untranslocated_app() {
        let translocated = Path::new(
            "/private/var/folders/x/AppTranslocation/1/d/OpenRows.app/Contents/MacOS/openrows-app",
        );
        assert_eq!(bridge_path(translocated), Err(AppError::Translocated));
        let missing = Path::new("/nonexistent-openrows/openrows-app");
        assert_eq!(bridge_path(missing), Err(AppError::BridgeMissing));
    }
}
