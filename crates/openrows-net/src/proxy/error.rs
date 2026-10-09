// SPDX-License-Identifier: Apache-2.0
//! Typed proxy failures; messages name the proxy and stage, never a password.

use std::{io, time::Duration};

use crate::Stage;

/// Every way going through a proxy can fail.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProxyError {
    /// A stage exceeded its budget.
    #[error("{stage} timed out after {after:?}")]
    Timeout {
        /// Stage that ran out of time.
        stage: Stage,
        /// Budget that was exceeded.
        after: Duration,
    },
    /// The proxy could not be resolved or reached.
    #[error("could not reach {proxy}")]
    Unreachable {
        /// `SOCKS5 proxy host:port`.
        proxy: String,
        /// Resolver or socket error.
        #[source]
        source: io::Error,
    },
    /// The proxy wants credentials and none are configured.
    #[error("{proxy} requires a user name and password")]
    AuthRequired {
        /// `SOCKS5 proxy host:port`.
        proxy: String,
    },
    /// The proxy rejected the configured credentials.
    #[error("{proxy} rejected the user name or password")]
    AuthFailed {
        /// `SOCKS5 proxy host:port`.
        proxy: String,
    },
    /// The proxy refused to open the destination.
    #[error("{proxy} could not open {destination}: {reason}")]
    Refused {
        /// `SOCKS5 proxy host:port`.
        proxy: String,
        /// `host:port` that was requested.
        destination: String,
        /// The proxy's reason (SOCKS reply or HTTP status line).
        reason: String,
    },
    /// The proxy answered with something that is not the expected protocol.
    #[error("{proxy} sent an invalid reply: {reason}")]
    Protocol {
        /// `SOCKS5 proxy host:port`.
        proxy: String,
        /// What was wrong.
        reason: String,
    },
    /// The destination cannot be expressed in the protocol (e.g. a host name
    /// longer than 255 bytes, or credentials longer than SOCKS5 allows).
    #[error("{reason}")]
    Invalid {
        /// What was wrong.
        reason: String,
    },
    /// Reading from or writing to the proxy failed mid-handshake.
    #[error("connection to {proxy} failed during the handshake")]
    Io {
        /// `SOCKS5 proxy host:port`.
        proxy: String,
        /// Socket error.
        #[source]
        source: io::Error,
    },
}

impl ProxyError {
    /// True when the user should change the proxy credentials.
    #[must_use]
    pub fn is_auth(&self) -> bool {
        matches!(self, Self::AuthRequired { .. } | Self::AuthFailed { .. })
    }
}
