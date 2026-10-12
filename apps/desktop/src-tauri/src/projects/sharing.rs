// SPDX-License-Identifier: Apache-2.0
//! Project connection sharing always passes through the v1 secret-free serializer.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri owns command arguments"
)]
use super::{commands::project, manifest};
use crate::connections::{Connections, IpcError};
use openrows_store::profile::{
    ConnectionProfile,
    exchange::{self, ImportBatch},
};
use std::io::Read as _;
use tauri::State;

/// Export explicitly selected local profiles to the project's portable definitions.
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn project_share_connections(
    state: State<'_, Connections>,
    root: String,
    ids: Vec<String>,
) -> Result<(), IpcError> {
    if ids.is_empty() {
        return Err(IpcError::invalid("Select connections to share"));
    }
    let mut project = project(&root)?;
    let profiles = crate::connections::exchange::selected(&state, &ids)?;
    save_connections(&mut project, &profiles)
}

fn save_connections(
    project: &mut super::ops::Project,
    profiles: &[ConnectionProfile],
) -> Result<(), IpcError> {
    let text = exchange::export(profiles)?;
    let relative = project
        .manifest
        .connections_file
        .clone()
        .unwrap_or_else(|| ".openrows/connections.json".into());
    let path = project.root.resolve(&relative)?;
    crate::queries::files::write_atomic(&path, text.as_bytes())?;
    project.manifest.connections_file = Some(relative);
    manifest::save(&project.root, &project.manifest)?;
    Ok(())
}

/// Read strictly validated v1 definitions for the ordinary import preview.
pub(crate) fn read_connections(root: &str) -> Result<ImportBatch, IpcError> {
    let project = project(root)?;
    let relative = project
        .manifest
        .connections_file
        .as_deref()
        .ok_or_else(|| IpcError::invalid("This project has no shared connections"))?;
    let path = project.root.resolve(relative)?;
    let file = std::fs::File::open(&path)
        .map_err(|_| IpcError::invalid("Cannot read project connections"))?;
    let mut text = String::new();
    file.take(4 * 1024 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(|_| IpcError::invalid("Project connections must be UTF-8 JSON"))?;
    if text.len() > 4 * 1024 * 1024 {
        return Err(IpcError::invalid("Project connections exceed 4 MiB"));
    }
    Ok(exchange::parse_strict(&text)?)
}

#[cfg(test)]
mod tests {
    use super::super::paths::ProjectRoot;
    use super::*;
    use std::fs;
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn fixture() -> Result<tempfile::TempDir, Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let root = ProjectRoot::new(dir.path())?;
        let mut manifest = manifest::load_or_create(&root)?;
        manifest.connections_file = Some(".openrows/connections.json".into());
        manifest::save(&root, &manifest)?;
        Ok(dir)
    }

    #[test]
    fn shared_definitions_use_v1_and_refuse_unsupported_secret_fields() -> TestResult {
        let dir = fixture()?;
        let path = dir.path().join(".openrows/connections.json");
        let root = dir.path().display().to_string();
        fs::write(&path, exchange::export(&[])?)?;
        assert!(read_connections(&root)?.entries.is_empty());
        fs::write(
            &path,
            r#"{"format":"openrows.connections","version":1,"profiles":[],"password":"synthetic"}"#,
        )?;
        assert!(read_connections(&root).is_err());
        fs::write(
            &path,
            r#"{"format":"openrows.connections","version":2,"profiles":[]}"#,
        )?;
        assert!(read_connections(&root).is_err());
        Ok(())
    }

    #[test]
    fn sharing_writes_portable_profiles_without_local_ids_or_opaque_values() -> TestResult {
        use openrows_store::profile::{EnvKind, EnvTag, ServerParams};
        let dir = fixture()?;
        let root = dir.path().display().to_string();
        let mut profile = ConnectionProfile::new_postgres(
            "team",
            ServerParams::default(),
            EnvTag::new(EnvKind::Dev),
        );
        profile.startup_commands = "SELECT 'synthetic-private-value'".into();
        profile
            .params
            .options
            .insert("token".into(), "synthetic-private-option".into());
        let id = serde_json::to_string(&profile.id)?;
        save_connections(&mut project(&root)?, &[profile])?;
        let text = fs::read_to_string(dir.path().join(".openrows/connections.json"))?;
        assert!(!text.contains(&id));
        assert!(!text.contains("synthetic-private"));
        assert!(!text.contains("secrets"));
        let batch = read_connections(&root)?;
        assert_eq!(batch.entries.len(), 1);
        assert_eq!(batch.entries[0].profile.name, "team");
        assert!(batch.entries[0].profile.imported);
        Ok(())
    }

    #[test]
    fn project_import_keeps_policy_upgrades_in_the_preview() -> TestResult {
        use openrows_store::profile::{EnvKind, EnvTag, Safety, ServerParams};
        let dir = fixture()?;
        let root = dir.path().display().to_string();
        let mut profile = ConnectionProfile::new_postgres(
            "production",
            ServerParams::default(),
            EnvTag::new(EnvKind::Prod),
        );
        profile.safety = Safety::Unrestricted;
        save_connections(&mut project(&root)?, &[profile])?;
        let batch = read_connections(&root)?;
        assert_eq!(batch.entries[0].profile.safety, Safety::ConfirmWrites);
        assert_eq!(batch.warnings.len(), 1);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn connections_cannot_escape_through_a_symlink() -> TestResult {
        let dir = fixture()?;
        let outside = tempfile::tempdir()?;
        let file = outside.path().join("connections.json");
        fs::write(&file, exchange::export(&[])?)?;
        std::os::unix::fs::symlink(file, dir.path().join(".openrows/connections.json"))?;
        assert!(read_connections(&dir.path().display().to_string()).is_err());
        Ok(())
    }
}
