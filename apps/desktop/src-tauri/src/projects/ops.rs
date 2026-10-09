// SPDX-License-Identifier: Apache-2.0
//! File operations of the project tree: new folder, new query, rename,
//! move, duplicate and delete. Every path is relative to the project root,
//! must stay inside one of its query folders, and never touches `.openrows`.
//! Nothing is overwritten: a target that exists is an error, and copies get
//! a free name (`name copy.sql`, `name copy 2.sql`, …).
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use super::error::ProjectError;
use super::manifest::{MANIFEST_DIR, Manifest};
use super::paths::{ProjectRoot, components, valid_name};
use super::tree::is_sql;

/// Most numbered names tried for a copy before giving up.
const MAX_COPIES: u32 = 1000;

/// A project with its manifest: the scope every operation is checked against.
pub(crate) struct Project {
    pub(crate) root: ProjectRoot,
    pub(crate) manifest: Manifest,
}

/// What a checked path may be.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Allow {
    /// A query folder itself or anything inside one (targets of new items).
    Folders,
    /// Strictly inside a query folder (items that are renamed, moved, deleted).
    Items,
}

impl Project {
    /// The absolute path of `relative` after the project rules.
    fn checked(&self, relative: &str, allow: Allow) -> Result<PathBuf, ProjectError> {
        let parts = components(relative)?;
        if parts.iter().any(|part| part == MANIFEST_DIR) {
            return Err(ProjectError::OutsideQueries(relative.to_owned()));
        }
        let inside = self.manifest.query_folders.iter().any(|folder| {
            let base = components(folder).unwrap_or_default();
            let deeper = parts.len() > base.len() || allow == Allow::Folders;
            deeper && parts.starts_with(&base)
        });
        if !inside {
            return Err(ProjectError::OutsideQueries(relative.to_owned()));
        }
        self.root.resolve(relative)
    }

    fn relative(&self, path: &Path) -> Result<String, ProjectError> {
        self.root
            .relative(path)
            .ok_or_else(|| ProjectError::Escapes(path.display().to_string()))
    }

    fn folder(&self, relative: &str) -> Result<PathBuf, ProjectError> {
        let path = self.checked(relative, Allow::Folders)?;
        if path.is_dir() {
            Ok(path)
        } else {
            Err(ProjectError::OutsideQueries(relative.to_owned()))
        }
    }

    /// Create folder `name` in `parent`; returns its path.
    pub(crate) fn create_folder(&self, parent: &str, name: &str) -> Result<String, ProjectError> {
        let target = self.folder(parent)?.join(valid_name(name)?);
        if target.exists() {
            return Err(ProjectError::Exists(self.relative(&target)?));
        }
        fs::create_dir(&target).map_err(|error| ProjectError::io(&target, &error))?;
        self.relative(&target)
    }

    /// Write `sql` to a new query file in `folder` named after `name` (a
    /// free name when taken); returns its path.
    pub(crate) fn create_query(
        &self,
        folder: &str,
        name: &str,
        sql: &str,
    ) -> Result<String, ProjectError> {
        let stem = valid_name(name)?;
        let stem = stem.strip_suffix(".sql").unwrap_or(stem);
        let target = free_name(&self.folder(folder)?, stem, "sql")?;
        let mut file =
            fs::File::create_new(&target).map_err(|error| ProjectError::io(&target, &error))?;
        file.write_all(sql.as_bytes())
            .map_err(|error| ProjectError::io(&target, &error))?;
        self.relative(&target)
    }

    /// Give an item a new name in its folder (queries keep `.sql`); returns its path.
    pub(crate) fn rename(&self, item: &str, name: &str) -> Result<String, ProjectError> {
        let source = self.checked(item, Allow::Items)?;
        let mut name = valid_name(name)?.to_owned();
        if source.is_file() && !is_sql(Path::new(&name)) {
            name.push_str(".sql");
        }
        let target = source
            .parent()
            .map(|parent| parent.join(&name))
            .ok_or_else(|| ProjectError::OutsideQueries(item.to_owned()))?;
        self.place(&source, &target)
    }

    /// Move an item into `folder`; returns its new path.
    pub(crate) fn move_to(&self, item: &str, folder: &str) -> Result<String, ProjectError> {
        let source = self.checked(item, Allow::Items)?;
        let parent = self.folder(folder)?;
        if source.is_dir() && parent.starts_with(&source) {
            return Err(ProjectError::IntoItself);
        }
        let name = source
            .file_name()
            .ok_or_else(|| ProjectError::OutsideQueries(item.to_owned()))?;
        self.place(&source, &parent.join(name))
    }

    fn place(&self, source: &Path, target: &Path) -> Result<String, ProjectError> {
        if !source.exists() {
            return Err(ProjectError::io(
                source,
                &std::io::ErrorKind::NotFound.into(),
            ));
        }
        // A case-only rename on a case-insensitive disk is the same entry.
        let same = fs::canonicalize(target).ok() == fs::canonicalize(source).ok();
        if target.exists() && !same {
            return Err(ProjectError::Exists(self.relative(target)?));
        }
        fs::rename(source, target).map_err(|error| ProjectError::io(target, &error))?;
        self.relative(target)
    }

    /// Copy an item beside itself under a free name; returns the copy's path.
    pub(crate) fn duplicate(&self, item: &str) -> Result<String, ProjectError> {
        let source = self.checked(item, Allow::Items)?;
        let parent = source
            .parent()
            .ok_or_else(|| ProjectError::OutsideQueries(item.to_owned()))?;
        let stem = source
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let target = if source.is_dir() {
            let target = free_name(parent, &format!("{stem} copy"), "")?;
            copy_folder(&source, &target)?;
            target
        } else {
            let target = free_name(parent, &format!("{stem} copy"), "sql")?;
            fs::copy(&source, &target).map_err(|error| ProjectError::io(&target, &error))?;
            target
        };
        self.relative(&target)
    }

    /// Delete a query file, or a folder with everything in it.
    pub(crate) fn delete(&self, item: &str) -> Result<(), ProjectError> {
        let path = self.checked(item, Allow::Items)?;
        let removed = if path.is_dir() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        removed.map_err(|error| ProjectError::io(&path, &error))
    }

    /// The absolute path of an existing item, for Reveal in Finder.
    pub(crate) fn existing(&self, item: &str) -> Result<PathBuf, ProjectError> {
        let path = self.checked(item, Allow::Folders)?;
        if path.exists() {
            Ok(path)
        } else {
            Err(ProjectError::io(
                &path,
                &std::io::ErrorKind::NotFound.into(),
            ))
        }
    }
}

/// `folder/stem.extension`, else `stem 2`, `stem 3`, … whichever is free.
fn free_name(folder: &Path, stem: &str, extension: &str) -> Result<PathBuf, ProjectError> {
    let named = |name: String| {
        if extension.is_empty() {
            folder.join(name)
        } else {
            folder.join(format!("{name}.{extension}"))
        }
    };
    let first = named(stem.to_owned());
    if !first.exists() {
        return Ok(first);
    }
    (2..MAX_COPIES)
        .map(|number| named(format!("{stem} {number}")))
        .find(|path| !path.exists())
        .ok_or_else(|| ProjectError::Exists(first.display().to_string()))
}

/// Copy the visible folders and `.sql` files of `source` into a new `target`.
fn copy_folder(source: &Path, target: &Path) -> Result<(), ProjectError> {
    fs::create_dir(target).map_err(|error| ProjectError::io(target, &error))?;
    let entries = fs::read_dir(source).map_err(|error| ProjectError::io(source, &error))?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let (from, to) = (entry.path(), target.join(&name));
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        if kind.is_dir() {
            copy_folder(&from, &to)?;
        } else if kind.is_file() && is_sql(&from) {
            fs::copy(&from, &to).map_err(|error| ProjectError::io(&to, &error))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(dir: &Path) -> Result<Project, ProjectError> {
        fs::create_dir_all(dir.join("queries/reports")).map_err(|e| ProjectError::io(dir, &e))?;
        Ok(Project {
            root: ProjectRoot::new(dir)?,
            manifest: Manifest::new("p"),
        })
    }

    #[test]
    fn creates_renames_moves_duplicates_and_deletes() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let p = project(dir.path())?;
        assert_eq!(p.create_folder("queries", "daily")?, "queries/daily");
        assert_eq!(
            p.create_query("queries", "Top users", "SELECT 1")?,
            "queries/Top users.sql"
        );
        assert_eq!(
            p.create_query("queries", "Top users.sql", "SELECT 2")?,
            "queries/Top users 2.sql"
        );
        assert_eq!(
            p.rename("queries/Top users 2.sql", "second")?,
            "queries/second.sql"
        );
        assert_eq!(
            p.move_to("queries/second.sql", "queries/daily")?,
            "queries/daily/second.sql"
        );
        assert_eq!(
            p.duplicate("queries/daily/second.sql")?,
            "queries/daily/second copy.sql"
        );
        assert_eq!(
            p.duplicate("queries/daily/second.sql")?,
            "queries/daily/second copy 2.sql"
        );
        assert_eq!(p.duplicate("queries/daily")?, "queries/daily copy");
        assert!(
            dir.path()
                .join("queries/daily copy/second copy.sql")
                .is_file()
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("queries/daily/second.sql"))?,
            "SELECT 2"
        );
        p.delete("queries/daily")?;
        assert!(!dir.path().join("queries/daily").exists());
        p.delete("queries/Top users.sql")?;
        assert!(!dir.path().join("queries/Top users.sql").exists());
        Ok(())
    }

    #[test]
    fn never_overwrites_or_moves_a_folder_into_itself() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let p = project(dir.path())?;
        p.create_query("queries", "a", "1")?;
        p.create_query("queries", "b", "2")?;
        assert!(matches!(
            p.rename("queries/a.sql", "b"),
            Err(ProjectError::Exists(_))
        ));
        assert!(matches!(
            p.create_folder("queries", "reports"),
            Err(ProjectError::Exists(_))
        ));
        p.create_folder("queries/reports", "inner")?;
        assert_eq!(
            p.move_to("queries/reports", "queries/reports/inner").err(),
            Some(ProjectError::IntoItself)
        );
        assert_eq!(fs::read_to_string(dir.path().join("queries/b.sql"))?, "2");
        Ok(())
    }

    #[test]
    fn paths_outside_the_query_folders_are_refused() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let p = project(dir.path())?;
        fs::create_dir_all(dir.path().join(".openrows"))?;
        fs::write(dir.path().join("README.sql"), "x")?;
        for (item, result) in [
            ("queries", p.delete("queries")),
            ("README.sql", p.delete("README.sql")),
            (".openrows", p.delete(".openrows")),
            ("../x", p.delete("../x")),
        ] {
            assert!(result.is_err(), "{item}");
        }
        assert!(p.create_folder(".openrows", "x").is_err());
        assert!(p.move_to("queries/reports", ".").is_err());
        assert!(dir.path().join("README.sql").is_file());
        Ok(())
    }
}
