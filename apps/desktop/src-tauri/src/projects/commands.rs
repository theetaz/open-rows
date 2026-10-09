// SPDX-License-Identifier: Apache-2.0
//! The `project_*` commands. The webview names a project by its absolute
//! root and items by paths relative to it; every command re-reads the
//! manifest, so only folders holding `.openrows/project.yaml` (created
//! through the native Open Project panel) can be touched.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri deserialises command arguments into owned values"
)]
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use specta::Type;
use tauri_plugin_dialog::{DialogExt as _, FilePath};

use super::error::ProjectError;
use super::manifest::{self, Manifest};
use super::ops::Project;
use super::paths::ProjectRoot;
use super::tree::{self, ProjectEntryDto};
use crate::connections::{IpcError, IpcErrorKind};

/// An open project: its manifest and its query tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectDto {
    /// Absolute, canonical project folder.
    root: String,
    name: String,
    /// Saved connection id project queries open on, when set.
    default_connection: Option<String>,
    query_folders: Vec<String>,
    entries: Vec<ProjectEntryDto>,
    /// The tree was cut at its size limit.
    truncated: bool,
}

impl ProjectDto {
    fn of(project: &Project) -> Self {
        let listing = tree::list(&project.root, &project.manifest);
        let Manifest {
            name,
            default_connection,
            query_folders,
            ..
        } = project.manifest.clone();
        Self {
            root: project.root.path().display().to_string(),
            name,
            default_connection,
            query_folders,
            entries: listing.entries,
            truncated: listing.truncated,
        }
    }
}

/// The project at `root` with its manifest.
fn project(root: &str) -> Result<Project, ProjectError> {
    let root = ProjectRoot::new(Path::new(root))?;
    let manifest = manifest::load(&root)?;
    Ok(Project { root, manifest })
}

/// Ask for a folder and open it as a project, creating
/// `.openrows/project.yaml` when it has none; `null` when cancelled.
#[tauri::command]
#[specta::specta]
pub(crate) async fn project_choose(
    window: tauri::WebviewWindow,
) -> Result<Option<ProjectDto>, IpcError> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Open Project")
        .pick_folder(move |picked: Option<FilePath>| {
            // The receiver only disappears when the command was dropped.
            let _ = sender.send(picked);
        });
    let picked = receiver.await.map_err(|_| {
        IpcError::new(
            IpcErrorKind::Internal,
            "The folder panel closed unexpectedly",
        )
    })?;
    let Some(folder) = picked else {
        return Ok(None);
    };
    let folder: PathBuf = folder
        .into_path()
        .map_err(|error| IpcError::invalid(error.to_string()))?;
    let root = ProjectRoot::new(&folder)?;
    let manifest = manifest::load_or_create(&root)?;
    Ok(Some(ProjectDto::of(&Project { root, manifest })))
}

/// Read a project (a recent one, or again after a change).
#[tauri::command]
#[specta::specta]
pub(crate) fn project_load(root: String) -> Result<ProjectDto, IpcError> {
    Ok(ProjectDto::of(&project(&root)?))
}

/// Set or clear the project's default connection.
#[tauri::command]
#[specta::specta]
pub(crate) fn project_set_default_connection(
    root: String,
    profile_id: Option<String>,
) -> Result<ProjectDto, IpcError> {
    let mut project = project(&root)?;
    project.manifest.default_connection = profile_id.filter(|id| !id.trim().is_empty());
    manifest::save(&project.root, &project.manifest)?;
    Ok(ProjectDto::of(&project))
}

/// New folder `name` inside `parent`; returns its path.
#[tauri::command]
#[specta::specta]
pub(crate) fn project_create_folder(
    root: String,
    parent: String,
    name: String,
) -> Result<String, IpcError> {
    Ok(project(&root)?.create_folder(&parent, &name)?)
}

/// New query file in `folder` holding `sql`; returns its path.
#[tauri::command]
#[specta::specta]
pub(crate) fn project_create_query(
    root: String,
    folder: String,
    name: String,
    sql: String,
) -> Result<String, IpcError> {
    Ok(project(&root)?.create_query(&folder, &name, &sql)?)
}

/// Rename a folder or query; returns its new path.
#[tauri::command]
#[specta::specta]
pub(crate) fn project_rename(root: String, path: String, name: String) -> Result<String, IpcError> {
    Ok(project(&root)?.rename(&path, &name)?)
}

/// Move a folder or query into `folder`; returns its new path.
#[tauri::command]
#[specta::specta]
pub(crate) fn project_move(root: String, path: String, folder: String) -> Result<String, IpcError> {
    Ok(project(&root)?.move_to(&path, &folder)?)
}

/// Copy a folder or query beside itself; returns the copy's path.
#[tauri::command]
#[specta::specta]
pub(crate) fn project_duplicate(root: String, path: String) -> Result<String, IpcError> {
    Ok(project(&root)?.duplicate(&path)?)
}

/// Delete a query, or a folder and everything in it.
#[tauri::command]
#[specta::specta]
pub(crate) fn project_delete(root: String, path: String) -> Result<(), IpcError> {
    Ok(project(&root)?.delete(&path)?)
}

/// Show a folder or query in the file manager (Finder selects it).
#[tauri::command]
#[specta::specta]
pub(crate) async fn project_reveal(root: String, path: String) -> Result<(), IpcError> {
    let target = project(&root)?.existing(&path)?;
    let status = tauri::async_runtime::spawn_blocking(move || reveal(&target).status())
        .await
        .map_err(|error| IpcError::new(IpcErrorKind::Internal, error.to_string()))?;
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(IpcError::new(
            IpcErrorKind::Internal,
            format!("The file manager exited with {status}"),
        )),
        Err(error) => Err(IpcError::new(IpcErrorKind::Internal, error.to_string())),
    }
}

/// The platform command that shows `path` selected in the file manager.
fn reveal(path: &Path) -> Command {
    if cfg!(target_os = "macos") {
        let mut command = Command::new("open");
        command.arg("-R").arg(path);
        command
    } else if cfg!(target_os = "windows") {
        let mut command = Command::new("explorer");
        command.arg(format!("/select,{}", path.display()));
        command
    } else {
        let mut command = Command::new("xdg-open");
        command.arg(path.parent().unwrap_or(path));
        command
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_work_on_projects_only() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let root = dir.path().display().to_string();
        let error = project_load(root.clone()).err().map(|error| error.kind);
        assert_eq!(error, Some(IpcErrorKind::NotFound));
        assert!(project_load("relative".to_owned()).is_err());
        manifest::load_or_create(&ProjectRoot::new(dir.path())?)?;
        let path = project_create_query(
            root.clone(),
            "queries".into(),
            "q".into(),
            "SELECT 1".into(),
        )?;
        let loaded = project_set_default_connection(root.clone(), Some("p1".into()))?;
        assert_eq!(loaded.default_connection.as_deref(), Some("p1"));
        assert_eq!(loaded.entries.len(), 2);
        assert_eq!(
            loaded.entries.get(1).map(|entry| entry.path.clone()),
            Some(path)
        );
        assert_eq!(
            project_load(root)?.default_connection.as_deref(),
            Some("p1")
        );
        Ok(())
    }
}
