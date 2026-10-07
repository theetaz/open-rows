// SPDX-License-Identifier: Apache-2.0
//! Typed failures of profile storage and URL parsing.
//!
//! No variant carries a password or the raw connection URL.

use std::path::PathBuf;

use openrows_secrets::SecretError;

/// Every way a profile operation can fail.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProfileError {
    /// Reading or writing a profile file failed.
    #[error("profile file operation on {path} failed")]
    Io {
        /// File or directory involved.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// A profile file is not valid profile JSON.
    #[error("profile file {path} is not a valid profile")]
    Json {
        /// File that failed to parse or serialise.
        path: PathBuf,
        /// Parser error (position only, no content).
        #[source]
        source: serde_json::Error,
    },
    /// The file was written by a newer app.
    #[error("profile format v{found} is newer than supported v{supported}")]
    UnsupportedFormat {
        /// Version in the file.
        found: u32,
        /// Newest version this build reads.
        supported: u32,
    },
    /// No profile with this id exists.
    #[error("profile {id} not found")]
    NotFound {
        /// Requested profile id.
        id: String,
    },
    /// A profile with this id already exists.
    #[error("profile {id} already exists")]
    AlreadyExists {
        /// Conflicting profile id.
        id: String,
    },
    /// A field failed validation.
    #[error("profile field {field} is invalid: {reason}")]
    Invalid {
        /// Offending field.
        field: &'static str,
        /// Why it was rejected.
        reason: &'static str,
    },
    /// The secret store failed.
    #[error("secret store failed")]
    Secret(#[from] SecretError),
}

/// Every way a `postgres://` URL can be rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum UrlError {
    /// The scheme is not `postgres` or `postgresql`.
    #[error("unsupported URL scheme {scheme:?}; expected postgres:// or postgresql://")]
    UnsupportedScheme {
        /// Scheme found (lower-cased).
        scheme: String,
    },
    /// The URL is not syntactically valid.
    #[error("malformed connection URL: {0}")]
    Malformed(#[from] ::url::ParseError),
    /// Multi-host failover URLs are not supported yet.
    #[error("multiple hosts in one URL are not supported")]
    MultipleHosts,
    /// A port is not a number in 1..=65535.
    #[error("invalid port {value:?}")]
    InvalidPort {
        /// The rejected port text.
        value: String,
    },
    /// Percent-decoding a component did not yield UTF-8.
    #[error("URL component {field} is not valid percent-encoded UTF-8")]
    InvalidEncoding {
        /// Component that failed to decode.
        field: &'static str,
    },
    /// `sslmode` is not a libpq mode.
    #[error("invalid sslmode {value:?}")]
    InvalidSslMode {
        /// The rejected mode.
        value: String,
    },
}
