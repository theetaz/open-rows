// SPDX-License-Identifier: Apache-2.0
//! One JSON file per personal profile: `<dir>/<id>.json` (docs/15 §3).

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use super::{ConnectionProfile, PROFILE_FORMAT, ProfileError, ProfileId};

/// Profiles read from disk plus files that could not be read, so one bad
/// file never hides the rest.
#[derive(Debug, Default)]
pub struct LoadedProfiles {
    /// Successfully parsed profiles, sorted by group then name.
    pub profiles: Vec<ConnectionProfile>,
    /// Unreadable profile files with the reason.
    pub unreadable: Vec<ProfileError>,
}

/// The personal `connections/` directory.
#[derive(Debug, Clone)]
pub struct ProfileFiles {
    dir: PathBuf,
}

impl ProfileFiles {
    /// Profiles under `dir`; the directory is created on first save.
    #[must_use]
    pub fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
        }
    }

    /// Path of the file for `id`. Ids are UUIDs, so no path traversal.
    #[must_use]
    pub fn path_for(&self, id: ProfileId) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    /// Whether a file exists for `id`.
    #[must_use]
    pub fn exists(&self, id: ProfileId) -> bool {
        self.path_for(id).is_file()
    }

    /// Atomically write `profile` (temp file + rename, mode 0600 on Unix).
    ///
    /// # Errors
    /// `Io` or `Json` errors when the file cannot be written.
    pub fn save(&self, profile: &ConnectionProfile) -> Result<(), ProfileError> {
        fs::create_dir_all(&self.dir).map_err(io_error(&self.dir))?;
        let path = self.path_for(profile.id);
        let json = serde_json::to_vec_pretty(profile).map_err(|source| ProfileError::Json {
            path: path.clone(),
            source,
        })?;
        let temp = path.with_extension("json.tmp");
        write_private(&temp, &json).map_err(io_error(&temp))?;
        fs::rename(&temp, &path).map_err(io_error(&path))
    }

    /// Read the profile `id`.
    ///
    /// # Errors
    /// `NotFound`, `Io`, `Json` or `UnsupportedFormat`.
    pub fn load(&self, id: ProfileId) -> Result<ConnectionProfile, ProfileError> {
        let path = self.path_for(id);
        match fs::read(&path) {
            Ok(bytes) => parse(&path, &bytes),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Err(ProfileError::NotFound { id: id.to_string() })
            }
            Err(source) => Err(ProfileError::Io { path, source }),
        }
    }

    /// Delete the file for `id`.
    ///
    /// # Errors
    /// `NotFound` or `Io`.
    pub fn delete(&self, id: ProfileId) -> Result<(), ProfileError> {
        let path = self.path_for(id);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Err(ProfileError::NotFound { id: id.to_string() })
            }
            Err(source) => Err(ProfileError::Io { path, source }),
        }
    }

    /// Read every `*.json` profile in the directory.
    ///
    /// # Errors
    /// `Io` when the directory exists but cannot be listed.
    pub fn load_all(&self) -> Result<LoadedProfiles, ProfileError> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(LoadedProfiles::default());
            }
            Err(source) => return Err(io_error(&self.dir)(source)),
        };
        let mut loaded = LoadedProfiles::default();
        for entry in entries {
            let path = entry.map_err(io_error(&self.dir))?.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                match fs::read(&path).map_err(io_error(&path)) {
                    Ok(bytes) => push_parsed(&mut loaded, parse(&path, &bytes)),
                    Err(error) => loaded.unreadable.push(error),
                }
            }
        }
        loaded
            .profiles
            .sort_by(|a, b| (&a.group, &a.name).cmp(&(&b.group, &b.name)));
        Ok(loaded)
    }
}

fn push_parsed(loaded: &mut LoadedProfiles, parsed: Result<ConnectionProfile, ProfileError>) {
    match parsed {
        Ok(profile) => loaded.profiles.push(profile),
        Err(error) => loaded.unreadable.push(error),
    }
}

fn parse(path: &Path, bytes: &[u8]) -> Result<ConnectionProfile, ProfileError> {
    let profile: ConnectionProfile =
        serde_json::from_slice(bytes).map_err(|source| ProfileError::Json {
            path: path.to_path_buf(),
            source,
        })?;
    if profile.openrows > PROFILE_FORMAT {
        return Err(ProfileError::UnsupportedFormat {
            found: profile.openrows,
            supported: PROFILE_FORMAT,
        });
    }
    Ok(profile)
}

fn io_error(path: &Path) -> impl Fn(io::Error) -> ProfileError + '_ {
    move |source| ProfileError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    fs::write(path, bytes)
}
