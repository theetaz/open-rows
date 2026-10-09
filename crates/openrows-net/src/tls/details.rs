// SPDX-License-Identifier: Apache-2.0
//! What a finished handshake negotiated, for diagnostics and the status popover.

use std::time::{Duration, SystemTime};

use rustls::{ClientConnection, ProtocolVersion};
use x509_cert::{
    Certificate, TbsCertificate,
    der::Decode as _,
    ext::pkix::{SubjectAltName, name::GeneralName},
};

use super::Verification;

const SECONDS_PER_DAY: u64 = 86_400;

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
    /// `notBefore` of the server certificate, e.g. `2026-01-01T00:00:00Z`.
    pub not_before: Option<String>,
    /// `notAfter` of the server certificate, e.g. `2027-01-01T00:00:00Z`.
    pub not_after: Option<String>,
    /// Whole days from the handshake until `notAfter`; negative once expired.
    pub expires_in_days: Option<i64>,
    /// Subject alternative names: DNS names and IP addresses.
    pub alt_names: Vec<String>,
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
            not_before: tbs.map(|tbs| tbs.validity().not_before.to_string()),
            not_after: tbs.map(|tbs| tbs.validity().not_after.to_string()),
            expires_in_days: tbs.map(|tbs| {
                days_until(tbs.validity().not_after.to_system_time(), SystemTime::now())
            }),
            alt_names: tbs.map(alt_names).unwrap_or_default(),
            verification,
        }
    }
}

/// Whole days from `now` until `expiry`, negative when it already passed.
pub(crate) fn days_until(expiry: SystemTime, now: SystemTime) -> i64 {
    let days = |duration: Duration| i64::try_from(duration.as_secs() / SECONDS_PER_DAY);
    match expiry.duration_since(now) {
        Ok(left) => days(left).unwrap_or(i64::MAX),
        Err(passed) => days(passed.duration()).map_or(i64::MIN, |days| -days - 1),
    }
}

/// DNS names and IP addresses of the subject alternative name extension.
fn alt_names(tbs: &TbsCertificate) -> Vec<String> {
    let Ok(Some((_, SubjectAltName(names)))) = tbs.get_extension::<SubjectAltName>() else {
        return Vec::new();
    };
    names
        .iter()
        .filter_map(|name| match name {
            GeneralName::DnsName(dns) => Some(dns.to_string()),
            GeneralName::IpAddress(octets) => ip_text(octets.as_bytes()),
            _ => None,
        })
        .collect()
}

fn ip_text(octets: &[u8]) -> Option<String> {
    if let Ok(v4) = <[u8; 4]>::try_from(octets) {
        return Some(std::net::Ipv4Addr::from(v4).to_string());
    }
    <[u8; 16]>::try_from(octets)
        .ok()
        .map(|v6| std::net::Ipv6Addr::from(v6).to_string())
}

fn protocol_name(version: ProtocolVersion) -> String {
    match version {
        ProtocolVersion::TLSv1_3 => "TLS 1.3".to_owned(),
        ProtocolVersion::TLSv1_2 => "TLS 1.2".to_owned(),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::days_until;

    #[test]
    fn days_until_counts_whole_days_and_goes_negative_after_expiry() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10 * 86_400);
        let later = |days: u64, extra: u64| now + Duration::from_secs(days * 86_400 + extra);
        assert_eq!(days_until(later(29, 3600), now), 29);
        assert_eq!(days_until(later(0, 10), now), 0);
        assert_eq!(days_until(now - Duration::from_secs(10), now), -1);
        assert_eq!(days_until(now - Duration::from_secs(2 * 86_400), now), -3);
    }
}
