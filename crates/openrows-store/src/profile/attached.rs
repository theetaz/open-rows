// SPDX-License-Identifier: Apache-2.0
//! Database files attached to every connection of a `SQLite` profile
//! (`ATTACH DATABASE … AS alias`), and the rules their aliases follow.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{FileParams, ProfileError};

/// Schemas `SQLite` reserves for the main and temporary databases.
const RESERVED: [&str; 2] = ["main", "temp"];

/// Field name of an attachment's alias in validation errors.
const ALIAS: &str = "file.attached.alias";

/// One attached database file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachedFile {
    /// Schema name the file's tables appear under.
    pub alias: String,
    /// Absolute path of an existing database file.
    pub path: PathBuf,
}

impl FileParams {
    /// Add `file` to the attached databases.
    ///
    /// # Errors
    /// [`ProfileError::Invalid`] when the alias or path breaks the rules of
    /// [`FileParams::validate_attached`] or the alias is already taken.
    pub fn attach(&mut self, file: AttachedFile) -> Result<(), ProfileError> {
        self.attached.push(file);
        let checked = self.validate_attached();
        if checked.is_err() {
            self.attached.pop();
        }
        checked
    }

    /// Remove the attached database named `alias` (case-insensitive).
    ///
    /// # Errors
    /// [`ProfileError::Invalid`] when no attached database has that alias.
    pub fn detach(&mut self, alias: &str) -> Result<AttachedFile, ProfileError> {
        let position = self
            .attached
            .iter()
            .position(|file| file.alias.eq_ignore_ascii_case(alias))
            .ok_or(ProfileError::Invalid {
                field: ALIAS,
                reason: "matches no attached database",
            })?;
        Ok(self.attached.remove(position))
    }

    /// Check every attachment: a non-empty alias without NUL that is not
    /// `main` or `temp` and is unique ignoring case, and an absolute path.
    ///
    /// # Errors
    /// [`ProfileError::Invalid`] naming the first problem.
    pub fn validate_attached(&self) -> Result<(), ProfileError> {
        for (index, file) in self.attached.iter().enumerate() {
            let taken = self.attached[..index]
                .iter()
                .any(|earlier| earlier.alias.eq_ignore_ascii_case(&file.alias));
            alias_problem(&file.alias, taken)
                .map(|reason| (ALIAS, reason))
                .or_else(|| {
                    (!file.path.is_absolute())
                        .then_some(("file.attached.path", "must be an absolute path"))
                })
                .map_or(Ok(()), |(field, reason)| {
                    Err(ProfileError::Invalid { field, reason })
                })?;
        }
        Ok(())
    }
}

/// Why `alias` cannot name an attached database; `taken` when an earlier
/// attachment already uses it.
fn alias_problem(alias: &str, taken: bool) -> Option<&'static str> {
    if alias.trim().is_empty() {
        Some("must not be empty")
    } else if alias.contains('\0') {
        Some("must not contain NUL characters")
    } else if RESERVED.iter().any(|name| name.eq_ignore_ascii_case(alias)) {
        Some("cannot be main or temp")
    } else if taken {
        Some("is already used by another attached database")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(alias: &str, path: &str) -> AttachedFile {
        AttachedFile {
            alias: alias.to_owned(),
            path: PathBuf::from(path),
        }
    }

    fn reason(result: &Result<(), ProfileError>) -> &'static str {
        match result {
            Err(ProfileError::Invalid { reason, .. }) => reason,
            _ => "ok",
        }
    }

    #[test]
    fn aliases_are_named_unique_and_not_reserved() {
        let mut params = FileParams::default();
        assert_eq!(reason(&params.attach(file("logs", "/data/logs.db"))), "ok");
        let cases = [
            (" ", "must not be empty"),
            ("a\0b", "must not contain NUL characters"),
            ("MAIN", "cannot be main or temp"),
            ("temp", "cannot be main or temp"),
            ("LOGS", "is already used by another attached database"),
        ];
        for (alias, expected) in cases {
            assert_eq!(reason(&params.attach(file(alias, "/data/x.db"))), expected);
        }
        let relative = params.attach(file("other", "x.db"));
        assert_eq!(reason(&relative), "must be an absolute path");
        assert_eq!(params.attached, vec![file("logs", "/data/logs.db")]);
    }

    #[test]
    fn detach_removes_by_alias_ignoring_case() -> Result<(), ProfileError> {
        let mut params = FileParams::default();
        params.attach(file("logs", "/data/logs.db"))?;
        assert_eq!(params.detach("Logs")?, file("logs", "/data/logs.db"));
        assert!(params.attached.is_empty());
        assert!(params.detach("logs").is_err());
        Ok(())
    }
}
