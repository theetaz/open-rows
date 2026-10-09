// SPDX-License-Identifier: Apache-2.0
//! Typed failures of SSH tunnel set-up and use.

use std::{io, path::PathBuf, time::Duration};

use crate::{ProxyError, Stage, known_hosts::KnownHostsLine};

/// Every way opening or using an SSH tunnel can fail.
///
/// Messages name the host, stage or file involved but never a secret.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SshError {
    /// A stage exceeded its budget (docs/14 §4).
    #[error("{stage} timed out after {after:?}")]
    Timeout {
        /// Stage that ran out of time.
        stage: Stage,
        /// Budget that was exceeded.
        after: Duration,
    },
    /// DNS resolution failed or returned no address.
    #[error("could not resolve {host}")]
    Resolve {
        /// Host name that failed to resolve.
        host: String,
        /// Resolver error.
        #[source]
        source: io::Error,
    },
    /// TCP connect to the first hop failed.
    #[error("could not connect to {host}:{port}")]
    Connect {
        /// Host that refused or was unreachable.
        host: String,
        /// Port that was dialled.
        port: u16,
        /// Socket error.
        #[source]
        source: io::Error,
    },
    /// The server's key differs from the recorded key: possible attack.
    #[error("host key for {host} changed ({fingerprint}); recorded at {at}")]
    HostKeyChanged {
        /// `host` or `[host]:port` as written in `known_hosts`.
        host: String,
        /// SHA256 fingerprint of the key the server presented.
        fingerprint: String,
        /// Where the conflicting key is recorded.
        at: KnownHostsLine,
    },
    /// The server presented a key marked `@revoked`.
    #[error("host key for {host} is revoked ({fingerprint}); see {at}")]
    HostKeyRevoked {
        /// `host` or `[host]:port`.
        host: String,
        /// SHA256 fingerprint of the presented key.
        fingerprint: String,
        /// The `@revoked` line.
        at: KnownHostsLine,
    },
    /// The first-use decision rejected an unknown host key.
    #[error("host key for {host} ({fingerprint}) was not accepted")]
    HostKeyRejected {
        /// `host` or `[host]:port`.
        host: String,
        /// SHA256 fingerprint of the presented key.
        fingerprint: String,
    },
    /// The server offered a host certificate; certificate trust is not implemented.
    #[error("host {host} presented a certificate; host certificates are not supported yet")]
    HostCertificateUnsupported {
        /// `host` or `[host]:port`.
        host: String,
    },
    /// Reading or writing a `known_hosts` file failed.
    #[error("could not access known_hosts file {path}")]
    KnownHostsIo {
        /// File that failed.
        path: PathBuf,
        /// I/O error.
        #[source]
        source: io::Error,
    },
    /// Every configured authentication method was rejected.
    #[error("authentication failed for {user}@{host} (tried: {tried})")]
    AuthFailed {
        /// Login user.
        user: String,
        /// Hop host.
        host: String,
        /// Comma-separated methods that were attempted.
        tried: String,
    },
    /// A private key file is encrypted and no passphrase was supplied.
    #[error("private key {path} is encrypted; a passphrase is required")]
    PassphraseRequired {
        /// Key file.
        path: PathBuf,
    },
    /// A private key file could not be read, decrypted or parsed.
    #[error("could not load private key {path}: {reason}")]
    KeyLoad {
        /// Key file.
        path: PathBuf,
        /// Cause, without key material.
        reason: String,
    },
    /// The ssh-agent socket is not configured.
    #[error("ssh-agent is not available: SSH_AUTH_SOCK is not set")]
    AgentNotConfigured,
    /// Talking to ssh-agent failed.
    #[error("ssh-agent request failed: {reason}")]
    Agent {
        /// Cause reported by the agent client.
        reason: String,
    },
    /// The user dismissed a keyboard-interactive prompt.
    #[error("keyboard-interactive authentication was cancelled")]
    PromptCancelled,
    /// The server rejected or failed a forwarded channel.
    #[error("could not open tunnel to {host}:{port}: {reason}")]
    ChannelOpen {
        /// Destination as seen from the hop.
        host: String,
        /// Destination port.
        port: u16,
        /// Server or protocol reason.
        reason: String,
    },
    /// The server refused or failed a forwarded Unix-socket channel.
    #[error("could not open the Unix socket {path} on the SSH server: {reason}")]
    SocketOpen {
        /// Socket path on the last hop.
        path: String,
        /// Server or protocol reason.
        reason: String,
    },
    /// The proxy in front of the first hop failed.
    #[error(transparent)]
    Proxy(#[from] ProxyError),
    /// A blocking helper task (file read, key decryption) panicked or was cancelled.
    #[error("a background task stopped unexpectedly")]
    BackgroundTask,
    /// A tunnel needs at least one hop.
    #[error("an SSH route needs at least one hop")]
    EmptyRoute,
    /// Any other SSH protocol or transport failure.
    #[error("SSH protocol error: {0}")]
    Protocol(#[from] russh::Error),
}
