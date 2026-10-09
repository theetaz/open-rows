// SPDX-License-Identifier: Apache-2.0
//! Cloud tools in the existing Settings ▸ Tools command.
use super::dto::{DumpToolDto, DumpToolKindDto};
use openrows_net::helper::tools::{self, TOOLS};
use std::path::PathBuf;
use tauri::Manager as _;

pub(super) async fn list(app: &tauri::AppHandle) -> Vec<DumpToolDto> {
    let settings = crate::settings::current(&app.state::<crate::settings::SettingsFile>());
    let path = std::env::var_os("PATH");
    let mut dirs = openrows_dump::search_dirs(path.as_deref());
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/bin"));
    }
    let mut result = Vec::new();
    for (key, name) in TOOLS {
        let override_path = settings
            .as_ref()
            .and_then(|value| value.pointer(&format!("/tools/{key}")))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(PathBuf::from);
        let found = tools::discover(name, override_path.as_deref(), &dirs).await;
        let (path, version, error) = match found {
            Ok(tool) => (
                Some(tool.path.display().to_string()),
                Some(tool.version),
                None,
            ),
            Err(error) => (None, None, Some(error.to_string())),
        };
        let tool = match name {
            "aws" => DumpToolKindDto::Aws,
            "session-manager-plugin" => DumpToolKindDto::SessionManagerPlugin,
            "kubectl" => DumpToolKindDto::Kubectl,
            "cloud-sql-proxy" => DumpToolKindDto::CloudSqlProxy,
            "alloydb-auth-proxy" => DumpToolKindDto::AlloydbAuthProxy,
            "cloudflared" => DumpToolKindDto::Cloudflared,
            _ => continue,
        };
        result.push(DumpToolDto {
            tool,
            name: name.into(),
            path,
            version,
            from_settings: override_path.is_some(),
            warning: None,
            error,
            others: Vec::new(),
        });
    }
    result
}
