// SPDX-License-Identifier: Apache-2.0
//! Connection setup: transport, the server greeting, TLS (an `SSLRequest`
//! packet, then the rustls handshake from `openrows-net`), the login and the
//! server identity, each stage timed in the [`ConnectTrace`].
use std::collections::HashMap;

use mysql_common::constants::CapabilityFlags;
use mysql_common::io::ParseBuf;
use mysql_common::packets::{AuthPlugin, HandshakePacket, HandshakeResponse, SslRequest};
use mysql_common::proto::MySerialize;
use openrows_driver_api::{
    ConnectRequest, ConnectStage, ConnectTrace, DriverError, SslMode, TlsStatus,
};
use openrows_net::tls::{TlsClient, failure_in};

use crate::auth::{Exchange, Secrets};
use crate::error::{err_packet, unexpected};
use crate::packet::Packets;
use crate::transport::{Target, within};

/// `utf8mb4_general_ci`, known to every supported `MySQL` and `MariaDB`.
const UTF8MB4: u8 = 45;

/// Largest packet the client accepts.
const MAX_PACKET: u32 = 64 * 1024 * 1024;

/// What the client asks for. Never `CLIENT_MULTI_STATEMENTS` (one statement
/// per call, so safe-mode classification holds), `CLIENT_LOCAL_FILES` or
/// compression. `CLIENT_FOUND_ROWS` makes `UPDATE` report matched rows, which
/// the edit guard relies on.
fn wanted() -> CapabilityFlags {
    CapabilityFlags::CLIENT_LONG_PASSWORD
        | CapabilityFlags::CLIENT_FOUND_ROWS
        | CapabilityFlags::CLIENT_LONG_FLAG
        | CapabilityFlags::CLIENT_PROTOCOL_41
        | CapabilityFlags::CLIENT_TRANSACTIONS
        | CapabilityFlags::CLIENT_SECURE_CONNECTION
        | CapabilityFlags::CLIENT_MULTI_RESULTS
        | CapabilityFlags::CLIENT_PS_MULTI_RESULTS
        | CapabilityFlags::CLIENT_PLUGIN_AUTH
        | CapabilityFlags::CLIENT_CONNECT_ATTRS
        | CapabilityFlags::CLIENT_PLUGIN_AUTH_LENENC_CLIENT_DATA
        | CapabilityFlags::CLIENT_DEPRECATE_EOF
}

/// A logged-in connection before the server-info query.
pub(crate) struct Established {
    pub(crate) packets: Packets,
    pub(crate) capabilities: CapabilityFlags,
    pub(crate) connection_id: u32,
    /// The greeting's version text, e.g. `8.4.3` or `5.5.5-10.11.6-MariaDB`.
    pub(crate) version: String,
    pub(crate) tls: TlsStatus,
}

/// The server greeting, owned.
struct Greeting {
    capabilities: CapabilityFlags,
    connection_id: u32,
    version: String,
    version_parsed: (u16, u16, u16),
    nonce: Vec<u8>,
    plugin: AuthPlugin<'static>,
}

/// Connect and log in. `sslmode=allow` retries with TLS when the plaintext
/// login is refused, as libpq does.
pub(crate) async fn establish(
    request: &ConnectRequest,
    target: &Target,
    trace: &mut ConnectTrace,
) -> Result<Established, DriverError> {
    let first = if target.tcp_like() {
        request.ssl_mode
    } else {
        SslMode::Disable
    };
    match attempt(request, target, first, trace).await {
        Err(DriverError::Auth(_) | DriverError::Server { .. }) if first == SslMode::Allow => {
            trace.restart();
            attempt(request, target, SslMode::Require, trace).await
        }
        result => result,
    }
}

async fn attempt(
    request: &ConnectRequest,
    target: &Target,
    mode: SslMode,
    trace: &mut ConnectTrace,
) -> Result<Established, DriverError> {
    let timeouts = request.timeouts;
    let stream = target.open(&timeouts, trace).await?;
    let mut packets = Packets::new(stream);
    let greeting = within(
        timeouts.auth,
        "reading the server greeting",
        greet(&mut packets),
    )
    .await?;
    // The SSL request and the login response must announce the same flags.
    let mut capabilities = greeting.capabilities & wanted();
    if !request.database.is_empty() {
        capabilities.insert(CapabilityFlags::CLIENT_CONNECT_WITH_DB);
    }
    let (mut packets, tls) =
        negotiate(packets, (&greeting, capabilities), mode, request, trace).await?;
    trace.tls = Some(tls.clone());
    if matches!(tls, TlsStatus::Encrypted(_)) {
        capabilities.insert(CapabilityFlags::CLIENT_SSL);
    }
    let secure = matches!(tls, TlsStatus::Encrypted(_)) || !target.tcp_like();
    let login = within(
        timeouts.auth,
        "authentication",
        log_in(&mut packets, &greeting, request, (capabilities, secure)),
    );
    trace
        .stage(ConnectStage::Authenticate, async {
            login.await.map_err(post_handshake)
        })
        .await?;
    Ok(Established {
        packets,
        capabilities,
        connection_id: greeting.connection_id,
        version: greeting.version,
        tls,
    })
}

async fn greet(packets: &mut Packets) -> Result<Greeting, DriverError> {
    let packet = packets.read().await?;
    if packet.first() == Some(&0xff) {
        return Err(err_packet(&packet, CapabilityFlags::empty())
            .unwrap_or_else(|| unexpected("as the server greeting", &packet)));
    }
    let greeting = ParseBuf(&packet)
        .parse::<HandshakePacket<'_>>(())
        .map_err(|error| DriverError::Protocol(format!("server greeting: {error}")))?;
    let version_parsed = greeting
        .maria_db_server_version_parsed()
        .or_else(|| greeting.server_version_parsed())
        .unwrap_or((0, 0, 0));
    Ok(Greeting {
        capabilities: greeting.capabilities(),
        connection_id: greeting.connection_id(),
        version: greeting.server_version_str().into_owned(),
        version_parsed,
        nonce: greeting.nonce(),
        plugin: greeting
            .auth_plugin()
            .map_or(AuthPlugin::MysqlNativePassword, AuthPlugin::into_owned),
    })
}

/// Ask for TLS when the mode wants it and the server offers it.
async fn negotiate(
    packets: Packets,
    (greeting, capabilities): (&Greeting, CapabilityFlags),
    mode: SslMode,
    request: &ConnectRequest,
    trace: &mut ConnectTrace,
) -> Result<(Packets, TlsStatus), DriverError> {
    let offered = greeting.capabilities.contains(CapabilityFlags::CLIENT_SSL);
    match mode {
        SslMode::Disable | SslMode::Allow => return Ok((packets, TlsStatus::Disabled)),
        SslMode::Prefer if !offered => return Ok((packets, TlsStatus::PlaintextFallback)),
        SslMode::Require | SslMode::VerifyCa | SslMode::VerifyFull if !offered => {
            return Err(DriverError::Tls(format!(
                "the server does not accept TLS connections, but sslmode={} requires TLS",
                mode.as_str()
            )));
        }
        _ => {}
    }
    let upgrade = within(
        request.timeouts.tls,
        "TLS negotiation",
        upgrade(packets, capabilities, mode, request),
    );
    trace.stage(ConnectStage::Tls, upgrade).await
}

async fn upgrade(
    mut packets: Packets,
    capabilities: CapabilityFlags,
    mode: SslMode,
    request: &ConnectRequest,
) -> Result<(Packets, TlsStatus), DriverError> {
    // A bad CA or key file fails before any byte goes out.
    let client = TlsClient::new(&request.tls.params(mode))
        .map_err(|error| DriverError::Tls(error.to_string()))?;
    let capabilities = capabilities | CapabilityFlags::CLIENT_SSL;
    let mut ssl_request = Vec::new();
    SslRequest::new(capabilities, MAX_PACKET, UTF8MB4).serialize(&mut ssl_request);
    packets.write(&ssl_request).await?;
    let (stream, sequence) = packets.into_parts();
    let host = match &request.endpoint {
        openrows_driver_api::Endpoint::Server { host, .. } => host.as_str(),
        openrows_driver_api::Endpoint::File { .. } => "",
    };
    let (stream, details) = client
        .handshake(stream, host)
        .await
        .map_err(|error| DriverError::Tls(error.to_string()))?;
    Ok((
        Packets::resume(Box::new(stream), sequence),
        TlsStatus::Encrypted(details),
    ))
}

async fn log_in(
    packets: &mut Packets,
    greeting: &Greeting,
    request: &ConnectRequest,
    (capabilities, secure): (CapabilityFlags, bool),
) -> Result<(), DriverError> {
    let password = request
        .password
        .as_ref()
        .map_or(&b""[..], |secret| secret.expose().as_bytes());
    let secrets = Secrets {
        password,
        secure,
        scramble: &greeting.nonce,
    };
    let (exchange, first) = Exchange::start(&greeting.plugin, &greeting.nonce, secrets)?;
    let attributes = HashMap::from([
        ("_client_name".to_owned(), "openrows".to_owned()),
        ("program_name".to_owned(), request.application_name.clone()),
    ]);
    let database = (!request.database.is_empty()).then_some(request.database.as_bytes());
    let response = HandshakeResponse::new(
        first,
        greeting.version_parsed,
        Some(request.user.as_bytes()),
        database,
        Some(greeting.plugin.clone()),
        capabilities,
        Some(attributes),
        MAX_PACKET,
    );
    let mut payload = Vec::new();
    response.serialize(&mut payload);
    packets.write(&payload).await?;
    exchange.finish(packets, secrets, capabilities).await
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
