// SPDX-License-Identifier: Apache-2.0
//! Handshake signatures made with the key of an X.509 v1 certificate.
//!
//! webpki parses only v3 certificates, but SQL Server's self-generated
//! certificate (used when no certificate is configured) is v1. Without
//! certificate verification the handshake signature is still checked: with
//! the public key read from the v1 certificate and the provider's
//! algorithms, so the server must still hold the key it presented.

use rustls::{
    CertificateError, DigitallySignedStruct, Error, client::danger::HandshakeSignatureValid,
    crypto::WebPkiSupportedAlgorithms, pki_types::CertificateDer,
};
use x509_cert::{
    Certificate,
    certificate::Version,
    der::{Decode as _, Encode as _},
};

/// `Some` result when `cert` is a v1 certificate; `None` for any other.
pub(super) fn verify_v1(
    message: &[u8],
    cert: &CertificateDer<'_>,
    dss: &DigitallySignedStruct,
    algorithms: &WebPkiSupportedAlgorithms,
) -> Option<Result<HandshakeSignatureValid, Error>> {
    let parsed = Certificate::from_der(cert.as_ref()).ok()?;
    let tbs = parsed.tbs_certificate();
    if tbs.version() != Version::V1 {
        return None;
    }
    let spki = tbs.subject_public_key_info();
    let algorithm = spki.algorithm.to_der().ok()?;
    let algorithm_id = contents(&algorithm)?;
    let key = spki.subject_public_key.as_bytes()?;
    let (_, candidates) = algorithms
        .mapping
        .iter()
        .find(|(scheme, _)| *scheme == dss.scheme)?;
    let verified = candidates
        .iter()
        .filter(|candidate| candidate.public_key_alg_id().as_ref() == algorithm_id)
        .any(|candidate| {
            candidate
                .verify_signature(key, message, dss.signature())
                .is_ok()
        });
    Some(if verified {
        Ok(HandshakeSignatureValid::assertion())
    } else {
        Err(Error::InvalidCertificate(CertificateError::BadSignature))
    })
}

/// The contents of a DER element (without its tag and length).
fn contents(der: &[u8]) -> Option<&[u8]> {
    let first = *der.get(1)?;
    let header = if first < 0x80 {
        2
    } else {
        2 + usize::from(first & 0x7F)
    };
    der.get(header..)
}

#[cfg(test)]
mod tests {
    use super::contents;

    #[test]
    fn der_contents_skip_short_and_long_lengths() {
        assert_eq!(contents(&[0x30, 2, 5, 0]), Some(&[5, 0][..]));
        assert_eq!(contents(&[0x30, 0x81, 1, 7]), Some(&[7][..]));
        assert_eq!(contents(&[0x30]), None);
    }
}
