// SPDX-License-Identifier: Apache-2.0
//! `known_hosts` verification (NFR-SEC-04): plain, wildcard and hashed entries,
//! `@revoked` markers, and a typed outcome for the first-use decision.
//!
//! [`check`] is pure over file text; [`KnownHosts`] adds the file I/O.

mod entry;
mod store;

use std::{
    fmt,
    path::{Path, PathBuf},
};

use russh::keys::{HashAlg, PublicKey};

use entry::{Marker, hosts_match, parse_entry};
pub use store::KnownHosts;

/// A line in a `known_hosts` file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnownHostsLine {
    /// File path.
    pub path: PathBuf,
    /// 1-based line number.
    pub line: usize,
}

impl fmt::Display for KnownHostsLine {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.path.display(), self.line)
    }
}

/// Verification outcome for a presented host key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostKeyStatus {
    /// A matching key is recorded.
    Trusted,
    /// No key of this algorithm is recorded: ask the user (first use).
    Unknown {
        /// `SHA256:…` fingerprint of the presented key.
        fingerprint: String,
        /// Algorithms already recorded for this host (a warning sign if any).
        other_algorithms: Vec<String>,
    },
    /// A different key of the same algorithm is recorded: hard stop.
    Changed {
        /// `SHA256:…` fingerprint of the presented key.
        fingerprint: String,
        /// The conflicting record.
        at: KnownHostsLine,
    },
    /// The presented key is marked `@revoked`: hard stop.
    Revoked {
        /// `SHA256:…` fingerprint of the presented key.
        fingerprint: String,
        /// The `@revoked` line.
        at: KnownHostsLine,
    },
}

/// `host` for port 22, otherwise `[host]:port`, lower-cased as OpenSSH does.
#[must_use]
pub fn host_port(host: &str, port: u16) -> String {
    let host = host.to_ascii_lowercase();
    if port == 22 {
        host
    } else {
        format!("[{host}]:{port}")
    }
}

/// OpenSSH-style `SHA256:<base64>` fingerprint.
#[must_use]
pub fn fingerprint(key: &PublicKey) -> String {
    key.fingerprint(HashAlg::Sha256).to_string()
}

/// Check `key` for `host_port` against the text of one `known_hosts` file.
#[must_use]
pub fn check(text: &str, source: &Path, host_port: &str, key: &PublicKey) -> HostKeyStatus {
    let mut trusted = false;
    let mut changed = None;
    let mut other_algorithms = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let Some(entry) = parse_entry(line).filter(|entry| hosts_match(entry.hosts, host_port))
        else {
            continue;
        };
        let same_key = entry.key.key_data() == key.key_data();
        let at = || KnownHostsLine {
            path: source.to_path_buf(),
            line: index + 1,
        };
        match entry.marker {
            Marker::Revoked if same_key => {
                return HostKeyStatus::Revoked {
                    fingerprint: fingerprint(key),
                    at: at(),
                };
            }
            Marker::Revoked | Marker::CertAuthority => {}
            Marker::None if same_key => trusted = true,
            Marker::None if entry.key.algorithm() == key.algorithm() => {
                changed.get_or_insert_with(at);
            }
            Marker::None => other_algorithms.push(entry.key.algorithm().to_string()),
        }
    }
    match (trusted, changed) {
        (true, _) => HostKeyStatus::Trusted,
        (false, Some(at)) => HostKeyStatus::Changed {
            fingerprint: fingerprint(key),
            at,
        },
        (false, None) => HostKeyStatus::Unknown {
            fingerprint: fingerprint(key),
            other_algorithms,
        },
    }
}

#[cfg(test)]
mod tests;
