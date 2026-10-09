// SPDX-License-Identifier: Apache-2.0
//! Opening a session: the stream (direct or through the route), `PRELOGIN`,
//! the TLS handshake inside `PRELOGIN` packets when encryption was agreed,
//! and `LOGIN7` with SQL Server authentication.
//!
//! The libpq `sslmode` maps onto the `PRELOGIN` encryption offer:
//! `disable` offers none; `allow` asks to encrypt the login only (the
//! server may insist on more); `prefer` and stronger ask to encrypt
//! everything, `prefer` falling back to plaintext when the server cannot.
//! Certificates are checked as the mode says (docs/12 §4.1).

use openrows_driver_api::{
    ConnectRequest, ConnectStage, ConnectTrace, DriverError, Endpoint, SslMode, TlsStatus,
};
use openrows_net::BoxStream;
use openrows_net::tls::{TlsClient, failure_in};
use tokio::io::AsyncWriteExt;

use crate::error::login_error;
use crate::reply::{Env, Ignore, read_reply};
use crate::tds::login::{self, Login};
use crate::tds::packet::{INITIAL_PACKET_SIZE, PacketType, frame};
use crate::tds::prelogin::{self, Encryption};
use crate::tds::tls::PreloginFraming;
use crate::tds::wire::Wire;
use crate::transport::{Target, within};

/// Packet size asked for at login; the server may choose another.
const PACKET_SIZE: u32 = 8000;

/// A logged-in connection.
pub(crate) struct Established {
    pub(crate) wire: Wire,
    pub(crate) env: Env,
    pub(crate) tls: TlsStatus,
}

/// How the transport ends up encrypted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Plan {
    Plain(PlainReason),
    /// TLS for the login packet only.
    LoginOnly,
    Full,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlainReason {
    Disabled,
    Fallback,
}

/// Connect and log in.
///
/// # Errors
/// `Network`, `Tls`, `Auth`, `Server` or `Protocol` failures.
pub(crate) async fn establish(
    request: &ConnectRequest,
    target: &Target,
    trace: &mut ConnectTrace,
) -> Result<Established, DriverError> {
    let mut stream = target.open(&request.timeouts, trace).await?;
    let offer = offer(request.ssl_mode);
    let exchange = within(
        request.timeouts.tls,
        "PRELOGIN",
        exchange_prelogin(&mut stream, offer),
    );
    let stage = if offer == Encryption::NotSupported {
        ConnectStage::Authenticate
    } else {
        ConnectStage::Tls
    };
    let reply = trace.stage(stage, exchange).await?;
    let plan = plan(request.ssl_mode, reply.encryption)?;
    let payload = login_payload(request);
    let (stream, tls, sent) = match plan {
        Plan::Plain(reason) => (stream, plain_status(reason), false),
        Plan::LoginOnly | Plan::Full => {
            let secure = encrypt(stream, request, (plan, &payload));
            let secured = within(request.timeouts.tls, "TLS handshake", secure);
            trace.stage(ConnectStage::Tls, secured).await?
        }
    };
    let login = within(
        request.timeouts.auth,
        "login",
        log_in(stream, &payload, sent),
    );
    let (wire, env) = trace.stage(ConnectStage::Authenticate, login).await?;
    Ok(Established { wire, env, tls })
}

fn offer(mode: SslMode) -> Encryption {
    match mode {
        SslMode::Disable => Encryption::NotSupported,
        SslMode::Allow => Encryption::Off,
        SslMode::Prefer | SslMode::Require | SslMode::VerifyCa | SslMode::VerifyFull => {
            Encryption::On
        }
    }
}

/// What the server's answer means for `mode`.
fn plan(mode: SslMode, server: Encryption) -> Result<Plan, DriverError> {
    match (mode, server) {
        (SslMode::Disable, Encryption::On | Encryption::Required) => Err(DriverError::Tls(
            "the server requires encryption; choose an encryption mode other than Disable"
                .to_owned(),
        )),
        (SslMode::Disable, _) | (SslMode::Allow, Encryption::NotSupported) => {
            Ok(Plan::Plain(PlainReason::Disabled))
        }
        (SslMode::Allow, Encryption::Off) => Ok(Plan::LoginOnly),
        (SslMode::Prefer, Encryption::NotSupported) => Ok(Plan::Plain(PlainReason::Fallback)),
        (_, Encryption::NotSupported) => Err(DriverError::Tls(format!(
            "the server does not support encryption, but sslmode={} requires it",
            mode.as_str()
        ))),
        _ => Ok(Plan::Full),
    }
}

fn plain_status(reason: PlainReason) -> TlsStatus {
    match reason {
        PlainReason::Disabled => TlsStatus::Disabled,
        PlainReason::Fallback => TlsStatus::PlaintextFallback,
    }
}

async fn exchange_prelogin(
    stream: &mut BoxStream,
    offer: Encryption,
) -> Result<prelogin::PreloginReply, DriverError> {
    let mut id = 1;
    let request = frame(
        PacketType::PreLogin,
        &prelogin::request(offer),
        INITIAL_PACKET_SIZE,
        &mut id,
    );
    stream.write_all(&request).await?;
    stream.flush().await?;
    let reply = read_message(stream).await?;
    prelogin::parse_reply(&reply)
}

/// One whole message read straight from the stream (before the wire exists).
async fn read_message(stream: &mut BoxStream) -> Result<Vec<u8>, DriverError> {
    use crate::tds::packet::{HEADER_LEN, Header};
    use tokio::io::AsyncReadExt;
    let mut message = Vec::new();
    loop {
        let mut header = [0_u8; HEADER_LEN];
        stream.read_exact(&mut header).await?;
        let header = Header::parse(header)?;
        let start = message.len();
        message.resize(start + header.payload, 0);
        stream.read_exact(&mut message[start..]).await?;
        if header.last {
            return Ok(message);
        }
    }
}

/// The TLS handshake inside `PRELOGIN` packets. With login-only
/// encryption the login packet goes out over TLS here and the session
/// continues in plaintext (`true` in the result: the login was sent).
async fn encrypt(
    stream: BoxStream,
    request: &ConnectRequest,
    (plan, login): (Plan, &[u8]),
) -> Result<(BoxStream, TlsStatus, bool), DriverError> {
    // A bad CA or key file fails before any byte goes out.
    let client = TlsClient::new(&request.tls.params(request.ssl_mode))
        .map_err(|error| DriverError::Tls(error.to_string()))?;
    let (mut stream, details) = client
        .handshake(PreloginFraming::new(stream), server_name(request))
        .await
        .map_err(|error| DriverError::Tls(error.to_string()))?;
    stream.get_mut().0.pass_through();
    if plan != Plan::LoginOnly {
        return Ok((Box::new(stream), TlsStatus::Encrypted(details), false));
    }
    let mut id = 1;
    let packets = frame(PacketType::Login, login, INITIAL_PACKET_SIZE, &mut id);
    stream.write_all(&packets).await?;
    stream.flush().await?;
    let (framing, _) = stream.into_inner();
    Ok((framing.into_inner(), TlsStatus::Disabled, true))
}

fn server_name(request: &ConnectRequest) -> &str {
    match &request.endpoint {
        Endpoint::Server { host, .. } => host,
        Endpoint::File { .. } => "",
    }
}

fn login_payload(request: &ConnectRequest) -> Vec<u8> {
    let password = request
        .password
        .as_ref()
        .map_or("", |secret| secret.expose());
    login::encode(&Login {
        user: &request.user,
        password,
        application: &request.application_name,
        server: server_name(request),
        database: &request.database,
        packet_size: PACKET_SIZE,
    })
}

/// Send `LOGIN7` (unless it went out with the TLS handshake) and read the
/// server's acknowledgement.
async fn log_in(stream: BoxStream, payload: &[u8], sent: bool) -> Result<(Wire, Env), DriverError> {
    let mut wire = Wire::new(stream);
    if sent {
        wire.expect_reply();
    } else {
        wire.send(PacketType::Login, payload).await?;
    }
    let mut env = Env::default();
    let outcome = read_reply(&mut wire, &mut env, &mut Ignore)
        .await
        .map_err(post_handshake)?;
    if let Some(error) = &outcome.error {
        return Err(login_error(error));
    }
    if outcome.login.is_none() {
        return Err(DriverError::Protocol(
            "the server ended the login without acknowledging it".to_owned(),
        ));
    }
    if let Some(size) = outcome.packet_size {
        wire.set_packet_size(size).await;
    }
    Ok((wire, env))
}

/// TLS 1.3 servers reject a client certificate on the first read after the
/// handshake, which surfaces as an I/O error during login: name it as TLS.
fn post_handshake(error: DriverError) -> DriverError {
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
    fn modes_offer_and_accept_encryption_like_libpq() {
        assert_eq!(offer(SslMode::Disable), Encryption::NotSupported);
        assert_eq!(offer(SslMode::Allow), Encryption::Off);
        assert_eq!(offer(SslMode::VerifyFull), Encryption::On);
        assert!(plan(SslMode::Disable, Encryption::Required).is_err());
        assert!(matches!(
            plan(SslMode::Allow, Encryption::Off),
            Ok(Plan::LoginOnly)
        ));
        assert!(matches!(
            plan(SslMode::Allow, Encryption::Required),
            Ok(Plan::Full)
        ));
        assert!(matches!(
            plan(SslMode::Prefer, Encryption::NotSupported),
            Ok(Plan::Plain(PlainReason::Fallback))
        ));
        assert!(plan(SslMode::Require, Encryption::NotSupported).is_err());
        assert!(matches!(
            plan(SslMode::VerifyFull, Encryption::Off),
            Ok(Plan::Full)
        ));
    }
}
