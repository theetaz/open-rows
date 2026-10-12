// SPDX-License-Identifier: Apache-2.0
//! Bounded SQL sources for completion and file-backed project snippets.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri owns command arguments"
)]
use super::{commands::project, manifest, tree};
use crate::connections::IpcError;
use serde::Serialize;
use specta::Type;
use std::io::Read as _;

/// A visible project query or snippet, read for editor completion.
#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectSqlDto {
    path: String,
    sql: String,
    snippet: bool,
}

/// Read visible SQL files with per-file and total bounds; never follow escaping links.
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn project_sql_items(root: String) -> Result<Vec<ProjectSqlDto>, IpcError> {
    let project = project(&root)?;
    let listing = tree::list(&project.root, &project.manifest);
    if listing.truncated {
        return Err(IpcError::invalid(
            "Project tree is too large for completion",
        ));
    }
    let mut items = Vec::new();
    let mut bytes = 0;
    for entry in listing
        .entries
        .iter()
        .filter(|e| e.kind == tree::ProjectEntryKind::Query)
    {
        let path = project.existing(&entry.path)?;
        let sql = read_sql(&path)?;
        bytes += sql.len();
        if sql.len() > 64 * 1024 || bytes > 4 * 1024 * 1024 || items.len() >= 500 {
            return Err(IpcError::invalid(
                "Project completion exceeds 500 files, 64 KiB per file or 4 MiB total",
            ));
        }
        let snippet = project
            .manifest
            .snippet_folders
            .iter()
            .any(|folder| entry.path.starts_with(&format!("{folder}/")) || folder == ".");
        items.push(ProjectSqlDto {
            path: entry.path.clone(),
            sql,
            snippet,
        });
    }
    Ok(items)
}

fn read_sql(path: &std::path::Path) -> Result<String, IpcError> {
    let mut sql = String::new();
    std::fs::File::open(path)
        .map_err(|_| IpcError::invalid("Cannot read project SQL"))?
        .take(64 * 1024 + 1)
        .read_to_string(&mut sql)
        .map_err(|_| IpcError::invalid("Project SQL must be UTF-8"))?;
    Ok(sql)
}

/// Save a snippet as a new .sql file, preserving existing files and their names.
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn project_create_snippet(
    root: String,
    name: String,
    sql: String,
) -> Result<String, IpcError> {
    super::paths::valid_name(&name)?;
    if sql.len() > 64 * 1024 {
        return Err(IpcError::invalid("Project snippets cannot exceed 64 KiB"));
    }
    let mut project = project(&root)?;
    if project.manifest.snippet_folders.is_empty() {
        let folder = "snippets";
        let path = project.root.resolve(folder)?;
        std::fs::create_dir_all(&path)
            .map_err(|_| IpcError::invalid("Cannot create snippet folder"))?;
        project.manifest.snippet_folders.push(folder.into());
        manifest::save(&project.root, &project.manifest)?;
    }
    let folder = project
        .manifest
        .snippet_folders
        .first()
        .ok_or_else(|| IpcError::invalid("No snippet folder"))?;
    Ok(project.create_query(folder, &name, &sql)?)
}

#[cfg(test)]
mod tests {
    use super::super::paths::ProjectRoot;
    use super::*;
    use std::fs;
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn snippets_are_sql_files_with_safe_tree_operations_and_no_overwrite() -> TestResult {
        let dir = tempfile::tempdir()?;
        manifest::load_or_create(&ProjectRoot::new(dir.path())?)?;
        let root = dir.path().display().to_string();
        let first = project_create_snippet(root.clone(), "find".into(), "SELECT 1".into())?;
        let second = project_create_snippet(root.clone(), "find".into(), "SELECT 2".into())?;
        assert_eq!(first, "snippets/find.sql");
        assert_eq!(second, "snippets/find 2.sql");
        assert_eq!(fs::read_to_string(dir.path().join(&first))?, "SELECT 1");
        let files = project_sql_items(root.clone())?;
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|file| file.snippet));
        let project = project(&root)?;
        assert_eq!(project.rename(&first, "renamed")?, "snippets/renamed.sql");
        project.delete("snippets/renamed.sql")?;
        assert!(project.delete("snippets").is_err());
        assert!(project_create_snippet(root, "../escape".into(), "SELECT 3".into()).is_err());
        Ok(())
    }

    #[test]
    fn oversized_completion_files_fail_visibly() -> TestResult {
        let dir = tempfile::tempdir()?;
        manifest::load_or_create(&ProjectRoot::new(dir.path())?)?;
        fs::write(
            dir.path().join("queries/large.sql"),
            "x".repeat(64 * 1024 + 1),
        )?;
        assert!(project_sql_items(dir.path().display().to_string()).is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn escaped_snippet_and_manifest_links_cannot_write_outside_the_project() -> TestResult {
        let dir = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        manifest::load_or_create(&ProjectRoot::new(dir.path())?)?;
        std::os::unix::fs::symlink(outside.path(), dir.path().join("snippets"))?;
        assert!(
            project_create_snippet(
                dir.path().display().to_string(),
                "escape".into(),
                "x".into()
            )
            .is_err()
        );
        assert!(!outside.path().join("escape.sql").exists());
        fs::remove_dir_all(dir.path().join(".openrows"))?;
        std::os::unix::fs::symlink(outside.path(), dir.path().join(".openrows"))?;
        assert!(manifest::load_or_create(&ProjectRoot::new(dir.path())?).is_err());
        assert!(!outside.path().join("project.yaml").exists());
        Ok(())
    }
}
