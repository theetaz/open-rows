// SPDX-License-Identifier: Apache-2.0
//! M1-07 details of a successful Test connection: the server certificate
//! (subject, issuer, validity, days to expiry, alternative names) and the
//! latency of a trivial query.
use std::time::{Duration, Instant};

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::{ConnectTrace, Connection, DriverError, ResultSink, SinkFlow, TlsStatus};
use openrows_net::tls::{TlsDetails, Verification};
use serde::Serialize;
use specta::Type;

/// Certificates expiring within this many days get a warning.
pub(crate) const EXPIRY_WARNING_DAYS: i64 = 30;
/// Round trips measured; the median is reported.
const SAMPLES: usize = 3;
/// Valid on every supported engine and answered without touching tables.
const PROBE: &str = "SELECT 1";

/// The server certificate as Test connection shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CertificateDto {
    subject: Option<String>,
    issuer: Option<String>,
    /// Validity start, e.g. `2026-01-01T00:00:00Z`.
    not_before: Option<String>,
    /// Validity end, e.g. `2027-01-01T00:00:00Z`.
    not_after: Option<String>,
    /// Whole days until `notAfter`; negative once expired.
    days_left: Option<i32>,
    /// Expired or expiring within 30 days.
    expiring_soon: bool,
    /// Subject alternative names (DNS names and IP addresses).
    alt_names: Vec<String>,
    /// The certificate was verified (chain, and host name when `hostVerified`).
    verified: bool,
    host_verified: bool,
}

/// The negotiated certificate of a traced connect, if TLS was used.
pub(crate) fn certificate(trace: &ConnectTrace) -> Option<CertificateDto> {
    let Some(TlsStatus::Encrypted(details)) = &trace.tls else {
        return None;
    };
    Some(CertificateDto {
        subject: details.subject.clone(),
        issuer: details.issuer.clone(),
        not_before: details.not_before.clone(),
        not_after: details.not_after.clone(),
        days_left: details
            .expires_in_days
            .map(|days| i32::try_from(days).unwrap_or(if days < 0 { i32::MIN } else { i32::MAX })),
        expiring_soon: expiry_note(details).is_some(),
        alt_names: details.alt_names.clone(),
        verified: !details.verification.is_skipped(),
        host_verified: details.verification == Verification::Full,
    })
}

/// "Certificate expires in 12 days" / "Certificate expired 3 days ago", or
/// `None` when it is valid for longer than the warning window.
pub(crate) fn expiry_note(details: &TlsDetails) -> Option<String> {
    let days = details.expires_in_days?;
    match days {
        _ if days >= EXPIRY_WARNING_DAYS => None,
        0 => Some("Certificate expires today".to_owned()),
        1.. => Some(format!("Certificate expires in {days} day{}", plural(days))),
        _ => Some(format!(
            "Certificate expired {} day{} ago",
            -days,
            plural(-days)
        )),
    }
}

fn plural(count: i64) -> &'static str {
    if count == 1 { "" } else { "s" }
}

/// Round trips of a trivial query.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LatencyDto {
    pub(crate) median_ms: f64,
    pub(crate) min_ms: f64,
    pub(crate) max_ms: f64,
    pub(crate) samples: u32,
}

impl LatencyDto {
    fn of(mut samples: Vec<Duration>) -> Self {
        samples.sort_unstable();
        let millis = |duration: Option<&Duration>| {
            duration.map_or(0.0, |duration| duration.as_secs_f64() * 1e3)
        };
        Self {
            median_ms: millis(samples.get(samples.len() / 2)),
            min_ms: millis(samples.first()),
            max_ms: millis(samples.last()),
            samples: u32::try_from(samples.len()).unwrap_or(u32::MAX),
        }
    }
}

/// Time [`SAMPLES`] round trips of `SELECT 1` on `connection`.
///
/// # Errors
/// The first failing query.
pub(crate) async fn latency(connection: &mut dyn Connection) -> Result<LatencyDto, DriverError> {
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        connection.execute(PROBE, &mut Discard).await?;
        samples.push(started.elapsed());
    }
    Ok(LatencyDto::of(samples))
}

/// Drops the probe's single row.
struct Discard;

impl ResultSink for Discard {
    fn begin_result(&mut self, _schema: SchemaRef) {}

    fn push_batch(&mut self, _batch: RecordBatch) -> SinkFlow {
        SinkFlow::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn details(days: Option<i64>) -> TlsDetails {
        TlsDetails {
            protocol: "TLS 1.3".to_owned(),
            cipher: "TLS13_AES_128_GCM_SHA256".to_owned(),
            subject: Some("CN=db.example.com".to_owned()),
            issuer: Some("CN=Example CA".to_owned()),
            not_before: Some("2026-01-01T00:00:00Z".to_owned()),
            not_after: Some("2026-11-01T00:00:00Z".to_owned()),
            expires_in_days: days,
            alt_names: vec!["db.example.com".to_owned(), "10.0.0.5".to_owned()],
            verification: Verification::Full,
        }
    }

    #[test]
    fn expiry_warns_inside_thirty_days_and_after_expiry() {
        assert_eq!(expiry_note(&details(Some(30))), None);
        assert_eq!(expiry_note(&details(None)), None);
        assert_eq!(
            expiry_note(&details(Some(29))).as_deref(),
            Some("Certificate expires in 29 days")
        );
        assert_eq!(
            expiry_note(&details(Some(1))).as_deref(),
            Some("Certificate expires in 1 day")
        );
        assert_eq!(
            expiry_note(&details(Some(0))).as_deref(),
            Some("Certificate expires today")
        );
        assert_eq!(
            expiry_note(&details(Some(-3))).as_deref(),
            Some("Certificate expired 3 days ago")
        );
    }

    #[test]
    fn certificates_come_from_encrypted_traces_only() {
        let trace = ConnectTrace {
            tls: Some(TlsStatus::Encrypted(details(Some(12)))),
            ..ConnectTrace::default()
        };
        let certificate = certificate(&trace).unwrap_or_else(|| unreachable!("encrypted"));
        assert_eq!(certificate.days_left, Some(12));
        assert!(certificate.expiring_soon && certificate.verified && certificate.host_verified);
        assert_eq!(certificate.alt_names, ["db.example.com", "10.0.0.5"]);
        let plain = ConnectTrace {
            tls: Some(TlsStatus::Disabled),
            ..ConnectTrace::default()
        };
        assert_eq!(super::certificate(&plain), None);
    }

    #[test]
    fn latency_reports_the_median_of_the_samples() {
        let samples = [4, 1, 2].map(Duration::from_millis).to_vec();
        let latency = LatencyDto::of(samples);
        assert!((latency.median_ms - 2.0).abs() < 1e-9);
        assert!((latency.min_ms - 1.0).abs() < 1e-9);
        assert!((latency.max_ms - 4.0).abs() < 1e-9);
        assert_eq!(latency.samples, 3);
    }
}
