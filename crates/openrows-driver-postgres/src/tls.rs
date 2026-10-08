// SPDX-License-Identifier: Apache-2.0
//! TLS negotiation (protocol §53.2.10): `SSLRequest`, then the rustls
//! handshake from `openrows-net`. Exactly one response byte is read before
//! the handshake, so no plaintext can be injected into the TLS session
//! (CVE-2021-23222).
use bytes::BytesMut;
use openrows_driver_api::DriverError;
use openrows_net::tls::{TlsClient, TlsError, failure_in};
use postgres_protocol::message::frontend;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::info::TlsStatus;
use crate::options::{ConnectOptions, SslMode};
use crate::wire::Io;

/// How the next attempt negotiates TLS.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Negotiation {
    /// No `SSLRequest` (`disable`, or `allow` before a rejection).
    Plaintext,
    /// Unix-domain socket: TLS does not apply.
    Socket,
    /// Ask; continue in plaintext if the server declines (`prefer`).
    Preferred,
    /// Ask; fail if the server declines.
    Required,
}

impl Negotiation {
    /// The first attempt for `mode` over a TCP-like (`tcp`) or socket target.
    pub(crate) fn first(mode: SslMode, tcp: bool) -> Self {
        match mode {
            _ if !tcp => Self::Socket,
            SslMode::Disable | SslMode::Allow => Self::Plaintext,
            SslMode::Prefer => Self::Preferred,
            SslMode::Require | SslMode::VerifyCa | SslMode::VerifyFull => Self::Required,
        }
    }

    /// True when this attempt sends an `SSLRequest`.
    pub(crate) fn asks(self) -> bool {
        matches!(self, Self::Preferred | Self::Required)
    }
}

/// Negotiate TLS on a fresh stream to `host` and return the stream to speak on.
///
/// # Errors
/// `Tls` for certificate files, a declined `Required` request or a failed
/// handshake; `Network` for transport errors; `Protocol` for a bad response.
pub(crate) async fn negotiate(
    mut stream: Box<dyn Io>,
    negotiation: Negotiation,
    options: &ConnectOptions,
    host: &str,
) -> Result<(Box<dyn Io>, TlsStatus), DriverError> {
    match negotiation {
        Negotiation::Plaintext => return Ok((stream, TlsStatus::Disabled)),
        Negotiation::Socket => return Ok((stream, TlsStatus::LocalSocket)),
        Negotiation::Preferred | Negotiation::Required => {}
    }
    // A bad CA or key file fails before any byte goes out.
    let client = TlsClient::new(&options.tls.params(options.ssl_mode)).map_err(tls_error)?;
    let mut request = BytesMut::with_capacity(8);
    frontend::ssl_request(&mut request);
    stream.write_all(&request).await?;
    stream.flush().await?;
    let mut answer = [0_u8; 1];
    stream.read_exact(&mut answer).await?;
    match answer[0] {
        b'S' => {
            let (stream, details) = client.handshake(stream, host).await.map_err(tls_error)?;
            Ok((Box::new(stream), TlsStatus::Encrypted(details)))
        }
        b'N' if negotiation == Negotiation::Preferred => Ok((stream, TlsStatus::PlaintextFallback)),
        b'N' => Err(DriverError::Tls(format!(
            "the server does not accept TLS connections, but sslmode={} requires TLS",
            options.ssl_mode.as_str()
        ))),
        other => Err(DriverError::Protocol(format!(
            "unexpected response {other:#04x} to SSLRequest"
        ))),
    }
}

#[expect(clippy::needless_pass_by_value, reason = "map_err adapter")]
fn tls_error(error: TlsError) -> DriverError {
    DriverError::Tls(error.to_string())
}

/// TLS 1.3 servers reject a client certificate on the client's first read,
/// which surfaces as an I/O error during startup: name it as a TLS failure.
pub(crate) fn post_handshake(error: DriverError) -> DriverError {
    match error {
        DriverError::Network(io) => match failure_in(&io) {
            Some(failure) => {
                DriverError::Tls(format!("the server ended the TLS session: {failure}"))
            }
            None => DriverError::Network(io),
        },
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_attempts_follow_the_mode_and_transport() {
        assert_eq!(
            Negotiation::first(SslMode::VerifyFull, false),
            Negotiation::Socket
        );
        assert_eq!(
            Negotiation::first(SslMode::Disable, true),
            Negotiation::Plaintext
        );
        assert_eq!(
            Negotiation::first(SslMode::Allow, true),
            Negotiation::Plaintext
        );
        assert_eq!(
            Negotiation::first(SslMode::Prefer, true),
            Negotiation::Preferred
        );
        for mode in [SslMode::Require, SslMode::VerifyCa, SslMode::VerifyFull] {
            assert_eq!(Negotiation::first(mode, true), Negotiation::Required);
        }
        assert!(Negotiation::Required.asks() && !Negotiation::Socket.asks());
    }

    async fn answer(byte: u8, negotiation: Negotiation) -> Result<TlsStatus, DriverError> {
        let (near, mut far) = tokio::io::duplex(64);
        let server = tokio::spawn(async move {
            let mut request = [0_u8; 8];
            far.read_exact(&mut request).await?;
            far.write_all(&[byte]).await?;
            Ok::<_, std::io::Error>(request)
        });
        let options = ConnectOptions {
            ssl_mode: SslMode::Require,
            ..ConnectOptions::default()
        };
        let result = negotiate(Box::new(near), negotiation, &options, "db.test").await;
        let request = server
            .await
            .map_err(|error| DriverError::Protocol(error.to_string()))??;
        assert_eq!(
            request,
            [0, 0, 0, 8, 4, 210, 22, 47],
            "SSLRequest code 80877103"
        );
        result.map(|(_, status)| status)
    }

    #[tokio::test]
    async fn a_declined_request_falls_back_only_when_preferred() {
        let fallback = answer(b'N', Negotiation::Preferred).await;
        assert_eq!(fallback.ok(), Some(TlsStatus::PlaintextFallback));
        let required = answer(b'N', Negotiation::Required).await;
        assert!(
            matches!(&required, Err(DriverError::Tls(message)) if message.contains("sslmode=require")),
            "{required:?}"
        );
        let garbage = answer(b'E', Negotiation::Required).await;
        assert!(
            matches!(garbage, Err(DriverError::Protocol(_))),
            "{garbage:?}"
        );
    }

    #[tokio::test]
    async fn plaintext_and_sockets_skip_negotiation() -> Result<(), DriverError> {
        let (near, _far) = tokio::io::duplex(8);
        let options = ConnectOptions::default();
        let (_, status) = negotiate(Box::new(near), Negotiation::Socket, &options, "h").await?;
        assert_eq!(status, TlsStatus::LocalSocket);
        Ok(())
    }

    #[test]
    fn transport_errors_without_tls_stay_network_errors() {
        let error = post_handshake(DriverError::Network(std::io::ErrorKind::BrokenPipe.into()));
        assert!(matches!(error, DriverError::Network(_)));
    }
}
