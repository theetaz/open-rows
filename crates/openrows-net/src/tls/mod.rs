// SPDX-License-Identifier: Apache-2.0
//! TLS for database routes (docs/12 §4.1, docs/24 §4): rustls with the ring
//! provider, TLS 1.2 or 1.3.
//!
//! Trust follows libpq: a configured CA file is the only trust anchor for
//! that connection; without one, [`Verification::Full`] uses the operating
//! system's trust store through `rustls-platform-verifier`. Protocol-specific
//! negotiation (`SSLRequest`, capability flags) stays in the drivers; they
//! hand a connected byte stream to [`TlsClient::handshake`].

mod cloud;
mod details;
mod error;
mod files;
mod legacy;
mod verify;

use std::{path::PathBuf, sync::Arc};

use rustls::{ClientConfig, pki_types::ServerName};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::{TlsConnector, client::TlsStream};

pub use cloud::CloudCa;
pub use details::TlsDetails;
pub use error::{HandshakeFailure, TlsError, failure_in};

use crate::SecretString;

/// How much of the server certificate is checked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Verification {
    /// Encrypt only: any certificate is accepted (libpq `require`, `prefer`,
    /// `allow`). Handshake signatures are still checked.
    None,
    /// The chain must lead to the configured CA; the host name is not
    /// checked (libpq `verify-ca`). Needs a CA file.
    Chain,
    /// Chain and host name (libpq `verify-full`).
    Full,
}

impl Verification {
    /// True when the server certificate is not authenticated at all.
    #[must_use]
    pub fn is_skipped(self) -> bool {
        self == Self::None
    }
}

/// Files and verification for one connection. `Debug` redacts the passphrase.
#[derive(Clone, Debug)]
pub struct TlsParams {
    /// Certificate check.
    pub verification: Verification,
    /// PEM CA bundle; replaces the system trust store when set.
    pub ca_file: Option<PathBuf>,
    /// A bundled cloud provider CA set; replaces the system trust store
    /// when set and there is no CA file.
    pub cloud_ca: Option<CloudCa>,
    /// PEM client certificate (chain) for certificate authentication.
    pub client_cert: Option<PathBuf>,
    /// PEM client private key: PKCS#8 (optionally encrypted), PKCS#1 or SEC1.
    pub client_key: Option<PathBuf>,
    /// Passphrase of an encrypted PKCS#8 client key.
    pub key_passphrase: Option<SecretString>,
}

impl TlsParams {
    /// Parameters with `verification` and no files.
    #[must_use]
    pub fn new(verification: Verification) -> Self {
        Self {
            verification,
            ca_file: None,
            cloud_ca: None,
            client_cert: None,
            client_key: None,
            key_passphrase: None,
        }
    }
}

/// A ready TLS client configuration: files are read and checked once.
#[derive(Clone)]
pub struct TlsClient {
    connector: TlsConnector,
    verification: Verification,
}

impl TlsClient {
    /// Read the CA, client certificate and key, and build the verifier.
    ///
    /// # Errors
    /// [`TlsError`] for unreadable or invalid files, a missing or wrong key
    /// passphrase, `Chain` without a CA file, or a platform trust failure.
    pub fn new(params: &TlsParams) -> Result<Self, TlsError> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let verifier = verify::server_verifier(params, &provider)?;
        let builder = ClientConfig::builder_with_provider(Arc::clone(&provider))
            .with_safe_default_protocol_versions()
            .map_err(|error| TlsError::Config(error.to_string()))?
            .dangerous()
            .with_custom_certificate_verifier(verifier);
        let config = match files::client_identity(params)? {
            Some(identity) => builder
                .with_client_auth_cert(identity.chain, identity.key)
                .map_err(|error| TlsError::ClientKey {
                    path: identity.key_path,
                    reason: error.to_string(),
                })?,
            None => builder.with_no_client_auth(),
        };
        Ok(Self {
            connector: TlsConnector::from(Arc::new(config)),
            verification: params.verification,
        })
    }

    /// The certificate check this client performs.
    #[must_use]
    pub fn verification(&self) -> Verification {
        self.verification
    }

    /// Run the TLS handshake over `stream`, checking the certificate for
    /// `host` (SNI is sent for DNS names, not for IP addresses).
    ///
    /// # Errors
    /// [`TlsError::InvalidServerName`] or [`TlsError::Handshake`].
    pub async fn handshake<S>(
        &self,
        stream: S,
        host: &str,
    ) -> Result<(TlsStream<S>, TlsDetails), TlsError>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let name =
            ServerName::try_from(host.to_owned()).map_err(|_| TlsError::InvalidServerName {
                name: host.to_owned(),
            })?;
        let stream = self
            .connector
            .connect(name, stream)
            .await
            .map_err(|error| TlsError::handshake(&error, host))?;
        let details = TlsDetails::of(stream.get_ref().1, self.verification);
        Ok((stream, details))
    }
}

#[cfg(test)]
mod tests;
