// SPDX-License-Identifier: Apache-2.0
use serde::Serialize;
use specta::Type;

#[derive(Debug, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppInfo {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) build_profile: BuildProfile,
}

#[derive(Debug, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum BuildProfile {
    Debug,
    Release,
}

#[tauri::command]
#[specta::specta]
pub(crate) fn app_info() -> AppInfo {
    AppInfo {
        name: "OpenRows".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        build_profile: if cfg!(debug_assertions) {
            BuildProfile::Debug
        } else {
            BuildProfile::Release
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_matches_package_and_profile() -> Result<(), serde_json::Error> {
        let info = serde_json::to_value(app_info())?;
        assert_eq!(info["name"], "OpenRows");
        assert_eq!(info["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(
            info["buildProfile"],
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
        );
        assert_eq!(info.as_object().map(serde_json::Map::len), Some(3));
        Ok(())
    }
}
