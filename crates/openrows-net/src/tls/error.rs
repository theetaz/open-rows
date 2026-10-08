// SPDX-License-Identifier: Apache-2.0
//! Typed TLS failures with actionable messages; never key material.

use std::{fmt, io, path::PathBuf};

use rustls::{CertificateError, Error as RustlsError};

/// Why the server's certificate or the handshake was rejected.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum HandshakeFailure {
    /// The chain does not lead to a trusted CA (or the configured CA file).
    UntrustedCertificate,
    /// The certificate is valid but not for the host name connected to.
    HostnameMismatch,
    /// The certificate has expired.
    Expired,
    /// The certificate is not valid yet (check the clock).
    NotYetValid,
    /// The certificate was revoked.
    Revoked,
    /// The server sent a fatal alert, e.g. because it rejected the client certificate.
    ServerAlert(String),
    /// Any other TLS protocol failure.
    Other(String),
}

/// Every way setting up TLS can fail.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TlsError {
    /// The CA file could not be read or holds no certificate.
    #[error("CA certificate file {path}: {reason}", path = path.display())]
    CaFile {
        /// The file.
        path: PathBuf,
        /// What is wrong, without file contents.
        reason: String,
    },
    /// `verify-ca` checks the chain against a CA file, and none is set.
    #[error(
        "Verify CA needs a CA certificate file; choose one, or use Verify full to trust the \
         system certificate store"
    )]
    ChainNeedsCa,
    /// The client certificate file could not be read or holds no certificate.
    #[error("client certificate file {path}: {reason}", path = path.display())]
    ClientCert {
        /// The file.
        path: PathBuf,
        /// What is wrong.
        reason: String,
    },
    /// The client key could not be read, decrypted or used.
    #[error("client key file {path}: {reason}", path = path.display())]
    ClientKey {
        /// The file.
        path: PathBuf,
        /// What is wrong, without key material.
        reason: String,
    },
    /// The client key is encrypted and no passphrase is stored.
    #[error("client key {path} is encrypted; enter its passphrase", path = path.display())]
    KeyPassphraseRequired {
        /// The key file.
        path: PathBuf,
    },
    /// The stored passphrase does not decrypt the client key.
    #[error("the passphrase does not decrypt client key {path}", path = path.display())]
    KeyPassphraseWrong {
        /// The key file.
        path: PathBuf,
    },
    /// A client certificate needs its key and vice versa.
    #[error("a client certificate and its key must be set together")]
    IncompleteClientIdentity,
    /// The host cannot be used as a TLS server name.
    #[error("{name} is not a valid TLS server name")]
    InvalidServerName {
        /// The host as configured.
        name: String,
    },
    /// The system trust store could not be loaded.
    #[error("the system certificate store is unavailable: {0}")]
    Platform(String),
    /// rustls rejected the configuration.
    #[error("TLS configuration rejected: {0}")]
    Config(String),
    /// The handshake failed.
    #[error("TLS handshake with {host} failed: {failure}")]
    Handshake {
        /// Host whose certificate was checked.
        host: String,
        /// Why it failed.
        failure: HandshakeFailure,
    },
    /// The transport failed during the handshake.
    #[error("TLS handshake with {host} failed: {source}")]
    Io {
        /// Host connected to.
        host: String,
        /// Transport error.
        #[source]
        source: io::Error,
    },
}

impl TlsError {
    /// Classify a handshake error from tokio-rustls.
    pub(crate) fn handshake(error: &io::Error, host: &str) -> Self {
        let rustls = error
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<RustlsError>());
        match rustls {
            Some(inner) => Self::Handshake {
                host: host.to_owned(),
                failure: classify(inner),
            },
            None => Self::Io {
                host: host.to_owned(),
                source: io::Error::new(error.kind(), error.to_string()),
            },
        }
    }
}

/// Map a rustls error onto what the user can act on.
pub(crate) fn classify(error: &RustlsError) -> HandshakeFailure {
    match error {
        RustlsError::InvalidCertificate(certificate) => classify_certificate(certificate),
        RustlsError::AlertReceived(alert) => HandshakeFailure::ServerAlert(format!("{alert:?}")),
        other => HandshakeFailure::Other(other.to_string()),
    }
}

fn classify_certificate(error: &CertificateError) -> HandshakeFailure {
    match error {
        CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. } => {
            HandshakeFailure::HostnameMismatch
        }
        CertificateError::Expired | CertificateError::ExpiredContext { .. } => {
            HandshakeFailure::Expired
        }
        CertificateError::NotValidYet | CertificateError::NotValidYetContext { .. } => {
            HandshakeFailure::NotYetValid
        }
        CertificateError::Revoked => HandshakeFailure::Revoked,
        CertificateError::UnknownIssuer
        | CertificateError::BadSignature
        | CertificateError::UnhandledCriticalExtension
        | CertificateError::InvalidPurpose
        | CertificateError::ApplicationVerificationFailure => {
            HandshakeFailure::UntrustedCertificate
        }
        other => HandshakeFailure::Other(format!("invalid certificate: {other:?}")),
    }
}

impl fmt::Display for HandshakeFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&describe(self))
    }
}

/// The TLS failure inside an I/O error raised after the handshake: TLS 1.3
/// servers reject a client certificate only when the client first reads.
#[must_use]
pub fn failure_in(error: &io::Error) -> Option<HandshakeFailure> {
    error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<RustlsError>())
        .map(classify)
}

fn describe(failure: &HandshakeFailure) -> String {
    match failure {
        HandshakeFailure::UntrustedCertificate => {
            "the server certificate is not signed by a trusted CA; choose its CA certificate \
             file, or use Require to encrypt without verification"
                .to_owned()
        }
        HandshakeFailure::HostnameMismatch => {
            "the server certificate is not valid for this host name; connect with the name \
             in the certificate, or use Verify CA"
                .to_owned()
        }
        HandshakeFailure::Expired => "the server certificate has expired".to_owned(),
        HandshakeFailure::NotYetValid => {
            "the server certificate is not valid yet; check the system clock".to_owned()
        }
        HandshakeFailure::Revoked => "the server certificate was revoked".to_owned(),
        HandshakeFailure::ServerAlert(alert) => {
            format!("the server ended the handshake ({alert}); check the client certificate")
        }
        HandshakeFailure::Other(reason) => reason.clone(),
    }
}
