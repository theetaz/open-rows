// SPDX-License-Identifier: Apache-2.0
//! Machine-local, non-exportable trust in Project MCP servers, bound to the
//! SHA-256 of each definition like credential-command trust
//! (`openrows-store` `CommandTrust`). A changed definition is untrusted again
//! automatically; its enabled tools and catalogue are discarded with it.
use super::{ClientError, ClientTool, ProjectServer};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::Write as _,
    path::{Path, PathBuf},
    sync::{Mutex, PoisonError},
};

/// Largest trust file read or written.
const FILE_LIMIT: usize = 1 << 20;

/// What the user approved for one Project server.
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrustEntry {
    /// Digest of the exact definition the user reviewed.
    pub digest: String,
    /// Tools the user enabled; all start disabled.
    pub enabled_tools: Vec<String>,
    /// Last explicitly discovered catalogue.
    pub tools: Vec<ClientTool>,
}

/// Current standing of a definition.
#[derive(Clone, PartialEq)]
pub enum TrustState {
    /// Never trusted on this device; its server never starts.
    Untrusted,
    /// Trusted earlier, but the definition changed: the old approval was removed.
    Changed,
    /// The exact definition the user trusted.
    Trusted(TrustEntry),
}

/// Private trust file outside the Project and the portable settings.
pub struct ProjectTrust {
    path: PathBuf,
    write: Mutex<()>,
}

impl ProjectTrust {
    /// Trust kept at `path` (owner-only).
    #[must_use]
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_owned(),
            write: Mutex::new(()),
        }
    }

    /// Standing of `server` under its device `key`; a stale approval is removed.
    /// # Errors
    /// The trust file cannot be read or rewritten.
    pub fn state(&self, key: &str, server: &ProjectServer) -> Result<TrustState, ClientError> {
        let _guard = self.lock();
        let mut entries = self.load()?;
        let Some(entry) = entries.get(key) else {
            return Ok(TrustState::Untrusted);
        };
        if entry.digest == server.digest() {
            return Ok(TrustState::Trusted(entry.clone()));
        }
        entries.remove(key);
        self.store(&entries)?;
        Ok(TrustState::Changed)
    }

    /// Trust the definition the user reviewed; refused if it changed since.
    /// # Errors
    /// Invalid or changed definition, or a failed write.
    pub fn trust(
        &self,
        key: &str,
        server: &ProjectServer,
        reviewed: &str,
    ) -> Result<(), ClientError> {
        server.validate()?;
        if server.digest() != reviewed {
            return Err(ClientError::Invalid);
        }
        self.update(|entries| {
            entries.insert(
                key.to_owned(),
                TrustEntry {
                    digest: reviewed.to_owned(),
                    ..TrustEntry::default()
                },
            );
            Ok(())
        })
    }

    /// Forget the approval, its enabled tools and catalogue.
    /// # Errors
    /// The trust file cannot be rewritten.
    pub fn revoke(&self, key: &str) -> Result<(), ClientError> {
        self.update(|entries| {
            entries.remove(key);
            Ok(())
        })
    }

    /// Change a trusted entry; untrusted or changed definitions are refused.
    /// # Errors
    /// Not trusted for this exact definition, or a failed write.
    pub fn edit(
        &self,
        key: &str,
        server: &ProjectServer,
        change: impl FnOnce(&mut TrustEntry),
    ) -> Result<TrustEntry, ClientError> {
        let digest = server.digest();
        let mut result = None;
        self.update(|entries| {
            let entry = entries
                .get_mut(key)
                .filter(|entry| entry.digest == digest)
                .ok_or(ClientError::Disabled)?;
            change(entry);
            result = Some(entry.clone());
            Ok(())
        })?;
        result.ok_or(ClientError::Disabled)
    }

    fn update(
        &self,
        change: impl FnOnce(&mut BTreeMap<String, TrustEntry>) -> Result<(), ClientError>,
    ) -> Result<(), ClientError> {
        let _guard = self.lock();
        let mut entries = self.load()?;
        change(&mut entries)?;
        self.store(&entries)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ()> {
        self.write.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn load(&self) -> Result<BTreeMap<String, TrustEntry>, ClientError> {
        match std::fs::read(&self.path) {
            Ok(bytes) if bytes.len() <= FILE_LIMIT => {
                serde_json::from_slice(&bytes).map_err(|_| ClientError::Invalid)
            }
            Ok(_) => Err(ClientError::Limit),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(_) => Err(ClientError::Invalid),
        }
    }

    fn store(&self, entries: &BTreeMap<String, TrustEntry>) -> Result<(), ClientError> {
        let bytes = serde_json::to_vec(entries).map_err(|_| ClientError::Invalid)?;
        if bytes.len() > FILE_LIMIT {
            return Err(ClientError::Limit);
        }
        write_private(&self.path, &bytes).map_err(|_| ClientError::Invalid)
    }
}

/// Owner-only atomic replacement.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let result = options
        .open(&temporary)
        .and_then(|mut file| file.write_all(bytes).and_then(|()| file.sync_all()))
        .and_then(|()| std::fs::rename(&temporary, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
#[path = "trust_tests.rs"]
mod tests;
