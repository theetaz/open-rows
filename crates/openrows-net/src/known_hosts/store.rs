// SPDX-License-Identifier: Apache-2.0
//! File-backed `known_hosts`: read the user's files plus an app-managed file,
//! and record first-use acceptances only in the app-managed file.

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use russh::keys::{Algorithm, PublicKey};

use super::{HostKeyStatus, check, entry::hosts_match, entry::parse_entry, host_port};
use crate::SshError;

/// The `known_hosts` files consulted for one connection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnownHosts {
    read_only: Vec<PathBuf>,
    learn_to: PathBuf,
}

impl KnownHosts {
    /// Consult `read_only` (e.g. `~/.ssh/known_hosts`, never modified) and
    /// `learn_to`, the app-managed file that accepted keys are appended to.
    #[must_use]
    pub fn new(read_only: Vec<PathBuf>, learn_to: PathBuf) -> Self {
        Self {
            read_only,
            learn_to,
        }
    }

    /// Verify `key` for `host:port` across every file.
    ///
    /// `@revoked` anywhere wins, then a trusted match, then a changed key.
    ///
    /// # Errors
    /// [`SshError::KnownHostsIo`] if an existing file cannot be read.
    pub fn verify(
        &self,
        host: &str,
        port: u16,
        key: &PublicKey,
    ) -> Result<HostKeyStatus, SshError> {
        let host_port = host_port(host, port);
        let statuses = self
            .texts()?
            .iter()
            .map(|(path, text)| check(text, path, &host_port, key))
            .collect();
        Ok(combine(statuses, key))
    }

    /// Host-key algorithms already recorded for `host:port`, so negotiation can
    /// prefer them (as OpenSSH does) instead of tripping a first-use prompt.
    ///
    /// # Errors
    /// [`SshError::KnownHostsIo`] if an existing file cannot be read.
    pub fn known_algorithms(&self, host: &str, port: u16) -> Result<Vec<Algorithm>, SshError> {
        let host_port = host_port(host, port);
        let mut algorithms: Vec<Algorithm> = Vec::new();
        for (_, text) in self.texts()? {
            let found = text
                .lines()
                .filter_map(parse_entry)
                .filter(|entry| hosts_match(entry.hosts, &host_port))
                .map(|entry| entry.key.algorithm());
            algorithms.extend(found);
        }
        let mut unique = Vec::with_capacity(algorithms.len());
        for algorithm in algorithms {
            if !unique.contains(&algorithm) {
                unique.push(algorithm);
            }
        }
        Ok(unique)
    }

    /// Append `key` for `host:port` to the app-managed file (created `0600`).
    ///
    /// # Errors
    /// [`SshError::KnownHostsIo`] if the file cannot be written.
    pub fn remember(&self, host: &str, port: u16, key: &PublicKey) -> Result<(), SshError> {
        let io_error = |source| SshError::KnownHostsIo {
            path: self.learn_to.clone(),
            source,
        };
        let existing = read_if_present(&self.learn_to)?.unwrap_or_default();
        let separator = if existing.is_empty() || existing.ends_with('\n') {
            ""
        } else {
            "\n"
        };
        let bare_key = PublicKey::new(key.key_data().clone(), "")
            .to_openssh()
            .map_err(|error| io_error(io::Error::other(error)))?;
        let line = format!("{separator}{} {bare_key}\n", host_port(host, port));
        append_private(&self.learn_to, line.as_bytes()).map_err(io_error)
    }

    /// Text of every existing file, user files first.
    fn texts(&self) -> Result<Vec<(&PathBuf, String)>, SshError> {
        let mut texts = Vec::new();
        for path in self.read_only.iter().chain(std::iter::once(&self.learn_to)) {
            if let Some(text) = read_if_present(path)? {
                texts.push((path, text));
            }
        }
        Ok(texts)
    }
}

fn combine(statuses: Vec<HostKeyStatus>, key: &PublicKey) -> HostKeyStatus {
    let rank = |status: &HostKeyStatus| match status {
        HostKeyStatus::Revoked { .. } => 0,
        HostKeyStatus::Trusted => 1,
        HostKeyStatus::Changed { .. } => 2,
        HostKeyStatus::Unknown { .. } => 3,
    };
    let other_algorithms = statuses
        .iter()
        .filter_map(|status| match status {
            HostKeyStatus::Unknown {
                other_algorithms, ..
            } => Some(other_algorithms.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    statuses
        .into_iter()
        .filter(|status| rank(status) < 3)
        .min_by_key(rank)
        .unwrap_or_else(|| HostKeyStatus::Unknown {
            fingerprint: super::fingerprint(key),
            other_algorithms,
        })
}

fn read_if_present(path: &Path) -> Result<Option<String>, SshError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(SshError::KnownHostsIo {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn append_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(bytes)
}
