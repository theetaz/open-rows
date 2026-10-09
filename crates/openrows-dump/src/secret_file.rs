// SPDX-License-Identifier: Apache-2.0
//! Temporary files readable only by the user (0600): the password for the
//! tools (never their command line or environment), CA bundles and
//! `pg_restore` use-lists. Each is deleted when dropped.
use std::io::Write as _;
use std::path::Path;

use tempfile::NamedTempFile;
use zeroize::Zeroizing;

use crate::error::DumpError;

/// A private temporary file, removed on drop.
#[derive(Debug)]
pub struct PrivateFile {
    file: NamedTempFile,
}

impl PrivateFile {
    /// Write `contents` to a new 0600 file named `openrows-<purpose>-…`.
    ///
    /// # Errors
    /// The temporary directory is not writable.
    pub fn new(purpose: &str, contents: &[u8]) -> Result<Self, DumpError> {
        let dir = std::env::temp_dir();
        let failed = |error| DumpError::io("cannot write a temporary file in", &dir, error);
        let prefix = format!("openrows-{purpose}-");
        let mut builder = tempfile::Builder::new();
        builder.prefix(&prefix);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            builder.permissions(std::fs::Permissions::from_mode(0o600));
        }
        let mut file = builder.tempfile().map_err(failed)?;
        file.write_all(contents).map_err(failed)?;
        file.as_file().sync_all().map_err(failed)?;
        Ok(Self { file })
    }

    /// A libpq password file (`PGPASSFILE`) matching any server for `password`.
    ///
    /// # Errors
    /// The password holds a line break, which the format cannot express, or
    /// the file cannot be written.
    pub fn pgpass(password: &str) -> Result<Self, DumpError> {
        if password.contains(['\n', '\r']) {
            return Err(DumpError::invalid(
                "Passwords with line breaks cannot be given to the PostgreSQL tools",
            ));
        }
        let escaped = escape(password, |c| match c {
            '\\' => Some(r"\\"),
            ':' => Some(r"\:"),
            _ => None,
        });
        let line = Zeroizing::new(format!("*:*:*:*:{}\n", escaped.as_str()));
        Self::new("pgpass", line.as_bytes())
    }

    /// A `MySQL` option file for `--defaults-extra-file` with `password`
    /// in its `[client]` group.
    ///
    /// # Errors
    /// The file cannot be written.
    pub fn mysql_options(password: &str) -> Result<Self, DumpError> {
        let escaped = escape(password, |c| match c {
            '\\' => Some(r"\\"),
            '"' => Some("\\\""),
            '\n' => Some(r"\n"),
            '\r' => Some(r"\r"),
            _ => None,
        });
        let text = Zeroizing::new(format!("[client]\npassword=\"{}\"\n", escaped.as_str()));
        Self::new("mysql", text.as_bytes())
    }

    /// Where the file is.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.file.path()
    }
}

/// `password` with characters replaced by `escape`, built in a buffer
/// large enough never to reallocate (so no unwiped copy is left behind).
fn escape(password: &str, escape: impl Fn(char) -> Option<&'static str>) -> Zeroizing<String> {
    let mut escaped = Zeroizing::new(String::with_capacity(password.len() * 2));
    for c in password.chars() {
        match escape(c) {
            Some(replacement) => escaped.push_str(replacement),
            None => escaped.push(c),
        }
    }
    escaped
}

#[cfg(test)]
#[path = "tests/secret_file.rs"]
mod tests;
