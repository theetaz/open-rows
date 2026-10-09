// SPDX-License-Identifier: Apache-2.0
//! Path rules of a project: the root is an absolute, existing folder, and
//! every path the webview sends is relative to it (`/`-separated) and may
//! not leave it, neither through `..` nor through a symbolic link.
use std::fs;
use std::path::{Component, Path, PathBuf};

use super::error::ProjectError;

/// Longest accepted file or folder name, in characters.
const MAX_NAME: usize = 200;

/// The canonical root folder of an open project.
#[derive(Clone, Debug)]
pub(crate) struct ProjectRoot {
    root: PathBuf,
}

impl ProjectRoot {
    /// The folder at `path`, which must be absolute and exist.
    pub(crate) fn new(path: &Path) -> Result<Self, ProjectError> {
        if !path.is_absolute() {
            return Err(ProjectError::NotAbsolute);
        }
        let root = fs::canonicalize(path).map_err(|error| ProjectError::io(path, &error))?;
        if !root.is_dir() {
            return Err(ProjectError::NotProject(path.display().to_string()));
        }
        Ok(Self { root })
    }

    /// The canonical root folder.
    pub(crate) fn path(&self) -> &Path {
        &self.root
    }

    /// The absolute path of `relative` (empty or `.` is the root itself).
    /// Rejects absolute paths, `..`, and paths whose existing part resolves
    /// outside the root through a symbolic link.
    pub(crate) fn resolve(&self, relative: &str) -> Result<PathBuf, ProjectError> {
        let parts = components(relative)?;
        let path = parts
            .iter()
            .fold(self.root.clone(), |path, part| path.join(part));
        let existing = path.ancestors().find(|ancestor| ancestor.exists());
        let real = existing.map(fs::canonicalize).transpose();
        match real {
            Ok(Some(real)) if real.starts_with(&self.root) => Ok(path),
            _ => Err(ProjectError::Escapes(relative.to_owned())),
        }
    }

    /// `path` relative to the root, `/`-separated; `None` outside the root.
    pub(crate) fn relative(&self, path: &Path) -> Option<String> {
        let rest = path.strip_prefix(&self.root).ok()?;
        let parts: Vec<String> = rest
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect();
        Some(parts.join("/"))
    }
}

/// The plain components of a relative path; `.` parts are dropped.
pub(crate) fn components(relative: &str) -> Result<Vec<String>, ProjectError> {
    let escapes = || ProjectError::Escapes(relative.to_owned());
    let mut parts = Vec::new();
    for part in Path::new(relative).components() {
        match part {
            Component::Normal(name) => {
                parts.push(name.to_str().ok_or_else(escapes)?.to_owned());
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(escapes());
            }
        }
    }
    Ok(parts)
}

/// A trimmed file or folder name without separators, or why it is unusable.
/// Names starting with `.` are refused: the project tree hides them.
pub(crate) fn valid_name(name: &str) -> Result<&str, ProjectError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ProjectError::InvalidName("Enter a name"));
    }
    if name.chars().count() > MAX_NAME {
        return Err(ProjectError::InvalidName(
            "Names have at most 200 characters",
        ));
    }
    if name.starts_with('.') {
        return Err(ProjectError::InvalidName("Names cannot start with a dot"));
    }
    if name.contains(['/', '\\', ':', '\0']) {
        return Err(ProjectError::InvalidName(
            "Names cannot contain /, \\ or : characters",
        ));
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_root_must_be_an_absolute_existing_folder() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            ProjectRoot::new(Path::new("relative/project")).err(),
            Some(ProjectError::NotAbsolute)
        );
        let dir = tempfile::tempdir()?;
        assert!(ProjectRoot::new(&dir.path().join("missing")).is_err());
        let file = dir.path().join("file.txt");
        fs::write(&file, "x")?;
        assert!(ProjectRoot::new(&file).is_err());
        assert!(ProjectRoot::new(dir.path()).is_ok());
        Ok(())
    }

    #[test]
    fn relative_paths_resolve_inside_the_root_only() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let root = ProjectRoot::new(dir.path())?;
        let inside = root.resolve("queries/./reports/new.sql")?;
        assert_eq!(inside, root.path().join("queries/reports/new.sql"));
        assert_eq!(root.resolve("")?, root.path());
        assert_eq!(
            root.relative(&inside).as_deref(),
            Some("queries/reports/new.sql")
        );
        for bad in ["../outside.sql", "queries/../../x", "/etc/passwd"] {
            assert_eq!(
                root.resolve(bad).err(),
                Some(ProjectError::Escapes(bad.to_owned())),
                "{bad}"
            );
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_links_out_of_the_root_are_refused() -> Result<(), Box<dyn std::error::Error>> {
        let outside = tempfile::tempdir()?;
        let dir = tempfile::tempdir()?;
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link"))?;
        let root = ProjectRoot::new(dir.path())?;
        assert!(root.resolve("link/secret.sql").is_err());
        assert!(root.resolve("link").is_err());
        Ok(())
    }

    #[test]
    fn names_are_trimmed_and_checked() {
        assert_eq!(valid_name("  Weekly report ").ok(), Some("Weekly report"));
        for bad in ["", "  ", ".hidden", "a/b", "a\\b", "a:b", &"x".repeat(201)] {
            assert!(valid_name(bad).is_err(), "{bad}");
        }
    }
}
