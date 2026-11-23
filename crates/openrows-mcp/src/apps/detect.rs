// SPDX-License-Identifier: Apache-2.0
//! Find an agent CLI on `PATH` and its usual install folders and read its
//! version. GUI apps start with a minimal `PATH`, so Homebrew, `~/.local/bin`,
//! npm/nvm/Volta/pnpm global folders and `~/.bun/bin` are probed after it.
//! No configuration, credential or browser file is read.
use super::{AgentApp, run};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

/// A located CLI and the version it reports.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AppInstall {
    /// Absolute executable path.
    pub path: String,
    /// Version from `--version`, when it answered.
    pub version: Option<String>,
}

/// Directories searched for agent CLIs, `PATH` first, without duplicates.
#[must_use]
pub fn search_directories() -> Vec<PathBuf> {
    let mut directories: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|value| std::env::split_paths(&value).collect())
        .unwrap_or_default();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        for folder in [
            ".local/bin",
            ".claude/local",
            ".bun/bin",
            ".npm-global/bin",
            ".volta/bin",
            "Library/pnpm",
        ] {
            directories.push(home.join(folder));
        }
        let nvm = std::env::var_os("NVM_DIR").map_or_else(|| home.join(".nvm"), PathBuf::from);
        directories.extend(node_versions(&nvm.join("versions/node")));
    }
    directories.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    let mut unique = Vec::with_capacity(directories.len());
    for directory in directories {
        if directory.is_absolute() && !unique.contains(&directory) {
            unique.push(directory);
        }
    }
    unique
}
/// `bin` folders of installed nvm Node versions, newest name first.
fn node_versions(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut versions: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("bin"))
        .take(32)
        .collect();
    versions.sort_unstable_by(|left, right| right.cmp(left));
    versions
}
fn names(app: AgentApp) -> Vec<String> {
    let base = app.id();
    if cfg!(windows) {
        vec![format!("{base}.exe"), format!("{base}.cmd")]
    } else {
        vec![base.to_owned()]
    }
}
/// The first executable for this app in [`search_directories`].
#[must_use]
pub fn locate(app: AgentApp) -> Option<PathBuf> {
    let names = names(app);
    search_directories()
        .iter()
        .flat_map(|directory| names.iter().map(move |name| directory.join(name)))
        .find(|path| executable(path))
}
pub(crate) fn executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    let runnable = {
        use std::os::unix::fs::PermissionsExt as _;
        metadata.permissions().mode() & 0o111 != 0
    };
    #[cfg(not(unix))]
    let runnable = true;
    path.is_absolute() && metadata.is_file() && runnable
}
/// Locate the app and ask its version; `None` when it is not installed.
pub async fn detect(app: AgentApp) -> Option<AppInstall> {
    let path = locate(app)?;
    let version = run(&path, &["--version".to_owned()], Duration::from_secs(8))
        .await
        .ok()
        .filter(|output| output.success)
        .and_then(|output| parse_version(&output.text));
    Some(AppInstall {
        path: path.to_string_lossy().into_owned(),
        version,
    })
}
/// The first dotted number, e.g. `2.1.330 (Claude Code)` or `codex-cli 0.171.0`.
pub(crate) fn parse_version(text: &str) -> Option<String> {
    text.split(|character: char| character.is_whitespace() || character == ',')
        .map(|word| word.trim_start_matches('v'))
        .find(|word| {
            let mut parts = word.split('.');
            parts.clone().count() >= 2
                && parts.all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
        })
        .map(|word| word.chars().take(32).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_parse_from_each_tool() {
        assert_eq!(
            parse_version("2.1.330 (Claude Code)\n").as_deref(),
            Some("2.1.330")
        );
        assert_eq!(
            parse_version("codex-cli 0.171.0").as_deref(),
            Some("0.171.0")
        );
        assert_eq!(parse_version("0.68.0\n").as_deref(), Some("0.68.0"));
        assert_eq!(parse_version("v1.2").as_deref(), Some("1.2"));
        assert_eq!(parse_version("no version here"), None);
    }

    #[test]
    fn search_covers_usual_install_folders_once() {
        let directories = search_directories();
        assert!(directories.contains(&PathBuf::from("/opt/homebrew/bin")));
        assert!(directories.iter().all(|path| path.is_absolute()));
        let mut unique = directories.clone();
        unique.dedup();
        assert_eq!(unique.len(), directories.len());
    }

    #[test]
    fn node_versions_are_newest_first() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        for version in ["v20.1.0", "v24.19.0"] {
            std::fs::create_dir_all(root.path().join(version).join("bin"))?;
        }
        let found = node_versions(root.path());
        assert!(found[0].ends_with("v24.19.0/bin"));
        assert_eq!(found.len(), 2);
        Ok(())
    }
}
