// SPDX-License-Identifier: Apache-2.0
//! The folder tree of a project's query folders: folders and `.sql` files,
//! depth first, folders before files, by name. Hidden entries (a leading
//! `.`) are skipped and symbolic links are not followed, so a link loop or
//! a link out of the project never shows up.
use std::fs;
use std::path::Path;

use serde::Serialize;
use specta::Type;

use super::manifest::Manifest;
use super::paths::ProjectRoot;

/// Most entries listed; a larger tree is cut and flagged as truncated.
const MAX_ENTRIES: usize = 10_000;
/// Deepest folder level listed below a query folder.
const MAX_DEPTH: usize = 16;

/// What a tree entry is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ProjectEntryKind {
    Folder,
    Query,
}

/// One folder or `.sql` file; `path` is relative to the project root and
/// `/`-separated. A query folder is listed first, then its contents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectEntryDto {
    pub(crate) path: String,
    pub(crate) kind: ProjectEntryKind,
}

/// The listed entries and whether the listing stopped at [`MAX_ENTRIES`].
pub(crate) struct Listing {
    pub(crate) entries: Vec<ProjectEntryDto>,
    pub(crate) truncated: bool,
}

/// Whether `path` names a `.sql` file.
pub(crate) fn is_sql(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("sql"))
}

/// Folders and `.sql` files of every query folder, in display order.
pub(crate) fn list(root: &ProjectRoot, manifest: &Manifest) -> Listing {
    let mut listing = Listing {
        entries: Vec::new(),
        truncated: false,
    };
    for folder in manifest
        .query_folders
        .iter()
        .chain(&manifest.snippet_folders)
    {
        let Ok(path) = root.resolve(folder) else {
            continue;
        };
        push(&mut listing, folder.clone(), ProjectEntryKind::Folder);
        walk(root, &path, 0, &mut listing);
    }
    listing
}

fn push(listing: &mut Listing, path: String, kind: ProjectEntryKind) -> bool {
    if listing.entries.iter().any(|entry| entry.path == path) {
        return true;
    }
    if listing.entries.len() >= MAX_ENTRIES {
        listing.truncated = true;
        return false;
    }
    listing.entries.push(ProjectEntryDto { path, kind });
    true
}

/// Visible children of `folder`: sub-folders first, then `.sql` files, by name.
fn children(folder: &Path) -> Vec<(String, bool)> {
    let Ok(read) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut items: Vec<(String, bool)> = read
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_owned();
            let kind = entry.file_type().ok()?;
            let folder = kind.is_dir();
            let query = kind.is_file() && is_sql(Path::new(&name));
            (!name.starts_with('.') && (folder || query)).then_some((name, folder))
        })
        .collect();
    items.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
    });
    items
}

fn walk(root: &ProjectRoot, folder: &Path, depth: usize, listing: &mut Listing) {
    if depth >= MAX_DEPTH {
        return;
    }
    for (name, is_folder) in children(folder) {
        let path = folder.join(&name);
        let Some(relative) = root.relative(&path) else {
            continue;
        };
        let kind = if is_folder {
            ProjectEntryKind::Folder
        } else {
            ProjectEntryKind::Query
        };
        if !push(listing, relative, kind) {
            return;
        }
        if is_folder {
            walk(root, &path, depth + 1, listing);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_folders_before_queries_and_skips_the_rest() -> Result<(), Box<dyn std::error::Error>> {
        use ProjectEntryKind::{Folder, Query};
        let dir = tempfile::tempdir()?;
        let base = dir.path().join("queries");
        fs::create_dir_all(base.join("reports/monthly"))?;
        fs::create_dir_all(base.join(".git"))?;
        fs::write(base.join("b.sql"), "SELECT 2")?;
        fs::write(base.join("A.SQL"), "SELECT 1")?;
        fs::write(base.join("notes.txt"), "x")?;
        fs::write(base.join(".hidden.sql"), "x")?;
        fs::write(base.join("reports/monthly/sales.sql"), "x")?;
        let root = ProjectRoot::new(dir.path())?;
        let listing = list(&root, &Manifest::new("p"));
        let paths: Vec<(&str, ProjectEntryKind)> = listing
            .entries
            .iter()
            .map(|entry| (entry.path.as_str(), entry.kind))
            .collect();
        assert_eq!(
            paths,
            vec![
                ("queries", Folder),
                ("queries/reports", Folder),
                ("queries/reports/monthly", Folder),
                ("queries/reports/monthly/sales.sql", Query),
                ("queries/A.SQL", Query),
                ("queries/b.sql", Query),
            ]
        );
        assert!(!listing.truncated);
        Ok(())
    }

    #[test]
    fn missing_and_escaping_folders_list_safely() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let root = ProjectRoot::new(dir.path())?;
        let mut manifest = Manifest::new("p");
        manifest.query_folders = vec!["queries".to_owned(), "../outside".to_owned()];
        let listing = list(&root, &manifest);
        assert_eq!(listing.entries.len(), 1);
        assert_eq!(
            listing.entries.first().map(|e| e.path.as_str()),
            Some("queries")
        );
        Ok(())
    }
}
