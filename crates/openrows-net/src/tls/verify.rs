// SPDX-License-Identifier: Apache-2.0
//! Server certificate verifiers for the three [`Verification`] levels.
//!
//! Every verifier checks handshake signatures with the provider's algorithms,
//! so even `None` proves the server holds the key of the certificate it sent.

use std::sync::Arc;

use rustls::{
    DigitallySignedStruct, Error, RootCertStore, SignatureScheme,
    client::{
        WebPkiServerVerifier,
        danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
        verify_server_cert_signed_by_trust_anchor,
    },
    crypto::{
        CryptoProvider, WebPkiSupportedAlgorithms, verify_tls12_signature, verify_tls13_signature,
    },
    pki_types::{CertificateDer, ServerName, UnixTime},
    server::ParsedCertificate,
};

use super::{TlsError, TlsParams, Verification, files};

/// The verifier for `params`: libpq semantics, a CA file replaces system trust.
pub(crate) fn server_verifier(
    params: &TlsParams,
    provider: &Arc<CryptoProvider>,
) -> Result<Arc<dyn ServerCertVerifier>, TlsError> {
    let algorithms = provider.signature_verification_algorithms;
    let roots = params.ca_file.as_deref().map(files::ca_roots).transpose()?;
    match (params.verification, roots) {
        (Verification::None, _) => Ok(Arc::new(Unverified { algorithms })),
        (Verification::Chain, Some(roots)) => Ok(Arc::new(ChainOnly {
            roots: Arc::new(roots),
            algorithms,
        })),
        (Verification::Chain, None) => Err(TlsError::ChainNeedsCa),
        (Verification::Full, Some(roots)) => {
            WebPkiServerVerifier::builder_with_provider(Arc::new(roots), Arc::clone(provider))
                .build()
                .map(|verifier| verifier as Arc<dyn ServerCertVerifier>)
                .map_err(|error| TlsError::Config(error.to_string()))
        }
        (Verification::Full, None) => rustls_platform_verifier::Verifier::new(Arc::clone(provider))
            .map(|verifier| Arc::new(verifier) as Arc<dyn ServerCertVerifier>)
            .map_err(|error| TlsError::Platform(error.to_string())),
    }
}

/// Accepts any certificate (encryption without authentication).
#[derive(Debug)]
struct Unverified {
    algorithms: WebPkiSupportedAlgorithms,
}

impl ServerCertVerifier for Unverified {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(message, cert, dss, &self.algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(message, cert, dss, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}

/// Checks the chain against the configured CA but not the host name.
#[derive(Debug)]
struct ChainOnly {
    roots: Arc<RootCertStore>,
    algorithms: WebPkiSupportedAlgorithms,
}

impl ServerCertVerifier for ChainOnly {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        let certificate = ParsedCertificate::try_from(end_entity)?;
        verify_server_cert_signed_by_trust_anchor(
            &certificate,
            &self.roots,
            intermediates,
            now,
            self.algorithms.all,
        )?;
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(message, cert, dss, &self.algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(message, cert, dss, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}
