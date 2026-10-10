// SPDX-License-Identifier: Apache-2.0
//! Offline copy-then-switch plans: no keychain, cache, log or unknown files.
use std::fs;
use std::path::{Path, PathBuf};

/// A failed validation, copy or pointer write leaves the original data authoritative.
#[derive(Debug, thiserror::Error)]
pub(crate) enum MoveError {
    #[error("{0}")]
    Invalid(String),
    #[error("Data directory I/O: {0}")]
    Io(#[from] std::io::Error),
}

/// Existing, writable, empty local directory outside bundles and the source tree.
pub(crate) fn validate(source: &Path, target: &Path) -> Result<PathBuf, MoveError> {
    if !target.is_absolute() {
        return Err(MoveError::Invalid("Choose an absolute directory".into()));
    }
    let source = fs::canonicalize(source)?;
    let target = fs::canonicalize(target)?;
    if !target.is_dir() || target.starts_with(&source) || source.starts_with(&target) {
        return Err(MoveError::Invalid(
            "Choose a separate empty directory".into(),
        ));
    }
    for part in target.components() {
        let name = part.as_os_str().to_string_lossy().to_lowercase();
        if Path::new(&name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("app"))
            || [
                "icloud",
                "dropbox",
                "onedrive",
                "google drive",
                "cloudstorage",
            ]
            .iter()
            .any(|cloud| name.contains(cloud))
        {
            return Err(MoveError::Invalid(
                "App bundles and recognised cloud-sync folders are not data locations".into(),
            ));
        }
    }
    if fs::read_dir(&target)?.next().is_some() {
        return Err(MoveError::Invalid("The destination must be empty".into()));
    }
    let probe = target.join(format!(".openrows-write-{}", std::process::id()));
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)?;
    fs::remove_file(probe)?;
    Ok(target)
}

/// Copy only documented local data, rolling back every created file on any error.
pub(crate) fn copy_then_switch(
    source: &Path,
    target: &Path,
    pointer: &Path,
) -> Result<(), MoveError> {
    validate(source, target)?;
    let staging = target.join(".openrows-copy");
    fs::create_dir(&staging)?;
    let mut created = Vec::new();
    let result = (|| {
        copy_data(source, &staging)?;
        for entry in fs::read_dir(&staging)? {
            let entry = entry?;
            let destination = target.join(entry.file_name());
            if destination.exists() {
                return Err(MoveError::Invalid(
                    "Destination changed during the copy".into(),
                ));
            }
            fs::rename(entry.path(), &destination)?;
            created.push(destination);
        }
        fs::remove_dir(&staging)?;
        write_pointer(pointer, target)
    })();
    if result.is_err() {
        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }
        for path in created {
            remove_owned(&path)?;
        }
    }
    result
}

fn copy_data(source: &Path, target: &Path) -> Result<(), MoveError> {
    for name in [
        "settings.json",
        "settings.schema.json",
        "openrows.sqlite",
        "connections",
        "known_hosts",
        "backups",
    ] {
        let from = source.join(name);
        if from.exists() {
            copy_entry(&from, &target.join(name))?;
        }
    }
    Ok(())
}

fn copy_entry(source: &Path, target: &Path) -> Result<(), MoveError> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        return Err(MoveError::Invalid(
            "Data directory contains a symbolic link; move it manually".into(),
        ));
    }
    if metadata.is_dir() {
        fs::create_dir(target)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_entry(&entry.path(), &target.join(entry.file_name()))?;
        }
    } else if metadata.is_file() {
        fs::copy(source, target)?;
    } else {
        return Err(MoveError::Invalid(
            "Data directory contains an unsupported file".into(),
        ));
    }
    Ok(())
}

/// Persist a pointer atomically; directory contents are already safely copied.
pub(crate) fn write_pointer(pointer: &Path, target: &Path) -> Result<(), MoveError> {
    let temporary = pointer.with_extension("tmp");
    let result = fs::write(&temporary, target.to_string_lossy().as_bytes())
        .and_then(|()| fs::rename(&temporary, pointer));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(Into::into)
}

fn remove_owned(path: &Path) -> Result<(), MoveError> {
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_paths_and_rejects_nested_bundle_cloud_and_occupied_targets()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let source = root.path().join("source");
        fs::create_dir(&source)?;
        for name in ["App.app/data", "Dropbox/data", "source/nested"] {
            let target = root.path().join(name);
            fs::create_dir_all(&target)?;
            assert!(validate(&source, &target).is_err());
        }
        assert!(validate(&source, Path::new("relative")).is_err());
        assert!(validate(&source, root.path()).is_err());
        let target = root.path().join("target");
        fs::create_dir(&target)?;
        assert_eq!(validate(&source, &target)?, fs::canonicalize(&target)?);
        fs::write(target.join("occupied"), "x")?;
        assert!(validate(&source, &target).is_err());
        Ok(())
    }

    #[test]
    fn copies_data_switches_only_after_success_and_never_moves_secrets()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let source = root.path().join("source");
        fs::create_dir(&source)?;
        let target = root.path().join("target");
        fs::create_dir(&target)?;
        fs::write(source.join("settings.json"), "{}")?;
        fs::write(source.join("keychain"), "test-only")?;
        fs::write(source.join("secrets.json"), "test-only")?;
        let pointer = root.path().join("location");
        copy_then_switch(&source, &target, &pointer)?;
        assert_eq!(fs::read_to_string(&pointer)?, target.to_string_lossy());
        assert!(source.join("settings.json").exists());
        assert!(target.join("settings.json").exists());
        assert!(!target.join("secrets.json").exists());
        assert!(!target.join("keychain").exists());
        Ok(())
    }

    #[test]
    fn switch_failure_rolls_back_the_copy_and_keeps_source()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let source = root.path().join("source");
        fs::create_dir(&source)?;
        let target = root.path().join("target");
        fs::create_dir(&target)?;
        fs::write(source.join("settings.json"), "{}")?;
        assert!(copy_then_switch(&source, &target, &root.path().join("missing/pointer")).is_err());
        assert_eq!(fs::read_dir(&target)?.count(), 0);
        assert!(source.join("settings.json").exists());
        Ok(())
    }
}
