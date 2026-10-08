// SPDX-License-Identifier: Apache-2.0
//! What a finished handshake negotiated, for diagnostics and the status popover.

use rustls::{ClientConnection, ProtocolVersion};
use x509_cert::{Certificate, der::Decode as _};

use super::Verification;

/// Negotiated protocol, cipher suite and the server certificate's identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TlsDetails {
    /// E.g. `TLS 1.3`.
    pub protocol: String,
    /// E.g. `TLS13_AES_256_GCM_SHA384`.
    pub cipher: String,
    /// Server certificate subject (RFC 4514), when it could be parsed.
    pub subject: Option<String>,
    /// Server certificate issuer (RFC 4514), when it could be parsed.
    pub issuer: Option<String>,
    /// `notAfter` of the server certificate, e.g. `2027-01-01T00:00:00Z`.
    pub not_after: Option<String>,
    /// The certificate check that was performed.
    pub verification: Verification,
}

impl TlsDetails {
    pub(crate) fn of(connection: &ClientConnection, verification: Verification) -> Self {
        let protocol = connection
            .protocol_version()
            .map_or_else(|| "TLS".to_owned(), protocol_name);
        let cipher = connection
            .negotiated_cipher_suite()
            .map(|suite| {
                let suite = suite.suite();
                suite
                    .as_str()
                    .map_or_else(|| format!("{suite:?}"), str::to_owned)
            })
            .unwrap_or_default();
        let certificate = connection
            .peer_certificates()
            .and_then(<[_]>::first)
            .and_then(|der| Certificate::from_der(der.as_ref()).ok());
        let tbs = certificate.as_ref().map(Certificate::tbs_certificate);
        Self {
            protocol,
            cipher,
            subject: tbs.map(|tbs| tbs.subject().to_string()),
            issuer: tbs.map(|tbs| tbs.issuer().to_string()),
            not_after: tbs.map(|tbs| tbs.validity().not_after.to_string()),
            verification,
        }
    }
}

fn protocol_name(version: ProtocolVersion) -> String {
    match version {
        ProtocolVersion::TLSv1_3 => "TLS 1.3".to_owned(),
        ProtocolVersion::TLSv1_2 => "TLS 1.2".to_owned(),
        other => format!("{other:?}"),
    }
}
