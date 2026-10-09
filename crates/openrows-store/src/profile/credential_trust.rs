// SPDX-License-Identifier: Apache-2.0
//! Machine-local, non-exportable trust keyed by profile id and command digest.
use super::{CredentialCommand, CredentialError, ProfileError, ProfileId};
use sha2::{Digest as _, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

/// A private trust file outside the exportable connections directory.
pub struct CommandTrust {
    path: PathBuf,
    write: Mutex<()>,
}
impl CommandTrust {
    /// Trust entries in the machine-local app configuration directory.
    #[must_use]
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_owned(),
            write: Mutex::new(()),
        }
    }
    /// Whether this exact program and argument vector is trusted for this id.
    /// # Errors
    /// The local trust file cannot be read.
    pub fn is_trusted(
        &self,
        id: ProfileId,
        command: &CredentialCommand,
    ) -> Result<bool, ProfileError> {
        Ok(self.load()?.get(&id.to_string()) == Some(&digest(command)?))
    }
    /// Persist trust only after an explicit confirmation of the exact invocation.
    /// # Errors
    /// The configuration directory or file cannot be written.
    pub fn trust(&self, id: ProfileId, command: &CredentialCommand) -> Result<(), ProfileError> {
        command.validate()?;
        let _guard = self.write.lock().map_err(|_| ProfileError::Invalid {
            field: "trust",
            reason: "store unavailable",
        })?;
        let mut entries = self.load()?;
        entries.insert(id.to_string(), digest(command)?);
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| self.io(source))?;
        }
        let bytes = serde_json::to_vec_pretty(&entries).map_err(|source| self.json(source))?;
        let temporary = self
            .path
            .with_extension(format!("{}.tmp", ProfileId::new()));
        write_private(&temporary, &bytes).map_err(|source| self.io(source))?;
        let result = fs::rename(&temporary, &self.path);
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.map_err(|source| self.io(source))
    }
    /// Refuse execution unless the current profile id and command match local trust.
    /// # Errors
    /// Untrusted or unreadable configuration.
    pub fn require(
        &self,
        id: ProfileId,
        command: &CredentialCommand,
    ) -> Result<(), CredentialError> {
        if self.is_trusted(id, command)? {
            Ok(())
        } else {
            Err(CredentialError::Untrusted)
        }
    }
    fn load(&self) -> Result<BTreeMap<String, String>, ProfileError> {
        match fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|source| self.json(source)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(source) => Err(self.io(source)),
        }
    }
    fn io(&self, source: std::io::Error) -> ProfileError {
        ProfileError::Io {
            path: self.path.clone(),
            source,
        }
    }
    fn json(&self, source: serde_json::Error) -> ProfileError {
        ProfileError::Json {
            path: self.path.clone(),
            source,
        }
    }
}
fn digest(command: &CredentialCommand) -> Result<String, ProfileError> {
    // Length-delimited JSON prevents ambiguous argument boundaries; timeout is not executable input.
    let bytes = serde_json::to_vec(&(&command.program, &command.args)).map_err(|source| {
        ProfileError::Json {
            path: PathBuf::from("credential command"),
            source,
        }
    })?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let result = (|| {
        let mut file = options.open(path)?;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    result
}
