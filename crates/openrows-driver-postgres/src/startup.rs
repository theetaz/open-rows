// SPDX-License-Identifier: Apache-2.0
//! Startup and authentication: cleartext, MD5 and SCRAM-SHA-256 (no channel binding).
use crate::wire::{Wire, server_error, unexpected};
use fallible_iterator::FallibleIterator;
use openrows_driver_api::DriverError;
use postgres_protocol::authentication::{md5_hash, sasl};
use postgres_protocol::message::backend::{AuthenticationSaslBody, Message};
use postgres_protocol::message::frontend;

/// Connection target and credentials; everything else is in
/// [`ConnectOptions`](crate::ConnectOptions). No `Debug`: it holds the password.
#[derive(Clone, Default, Eq, PartialEq)]
pub struct PgConfig {
    /// Host name, IP address, or a Unix socket directory (starts with `/`).
    pub host: String,
    /// TCP port; also names the socket file `.s.PGSQL.<port>`.
    pub port: u16,
    /// Role name.
    pub user: String,
    /// Password, if the server asks for one.
    pub password: Option<String>,
    /// Database name; empty lets the server default it to the role name.
    pub database: String,
}

/// Process id and secret from `BackendKeyData`, needed for `CancelRequest`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BackendKey {
    pub(crate) process_id: i32,
    pub(crate) secret_key: i32,
}

/// What startup learned besides "authenticated".
pub(crate) struct Started {
    pub(crate) key: Option<BackendKey>,
    /// `server_version` parameter status; empty if the server sent none.
    pub(crate) server_version: String,
}

pub(crate) async fn startup(
    wire: &mut Wire,
    config: &PgConfig,
    application_name: &str,
) -> Result<Started, DriverError> {
    let mut parameters = vec![
        ("user", config.user.as_str()),
        ("client_encoding", "UTF8"),
        ("application_name", application_name),
    ];
    if !config.database.is_empty() {
        parameters.push(("database", config.database.as_str()));
    }
    frontend::startup_message(parameters, wire.out())?;
    wire.flush().await?;
    authenticate(wire, config).await?;
    let mut started = Started {
        key: None,
        server_version: String::new(),
    };
    loop {
        match wire.next().await? {
            Message::ReadyForQuery(_) => return Ok(started),
            Message::BackendKeyData(body) => {
                started.key = Some(BackendKey {
                    process_id: body.process_id(),
                    secret_key: body.secret_key(),
                });
            }
            Message::ParameterStatus(body) if body.name()? == "server_version" => {
                body.value()?.clone_into(&mut started.server_version);
            }
            Message::ParameterStatus(_) | Message::NoticeResponse(_) => {}
            Message::ErrorResponse(body) => return Err(server_error(&body)),
            _ => return Err(unexpected("startup")),
        }
    }
}

async fn authenticate(wire: &mut Wire, config: &PgConfig) -> Result<(), DriverError> {
    loop {
        match wire.next().await? {
            Message::AuthenticationOk => return Ok(()),
            Message::AuthenticationCleartextPassword => {
                frontend::password_message(password(config)?, wire.out())?;
            }
            Message::AuthenticationMd5Password(body) => {
                let hash = md5_hash(config.user.as_bytes(), password(config)?, body.salt());
                frontend::password_message(hash.as_bytes(), wire.out())?;
            }
            Message::AuthenticationSasl(body) => scram(wire, config, &body).await?,
            Message::ErrorResponse(body) => {
                return Err(DriverError::Auth(server_error(&body).to_string()));
            }
            _ => return Err(DriverError::Auth("unsupported method".to_owned())),
        }
        wire.flush().await?;
    }
}

async fn scram(
    wire: &mut Wire,
    config: &PgConfig,
    body: &AuthenticationSaslBody,
) -> Result<(), DriverError> {
    let mut mechanisms = body.mechanisms();
    let mut offered = false;
    while let Some(mechanism) = mechanisms.next().map_err(auth)? {
        offered |= mechanism == sasl::SCRAM_SHA_256;
    }
    if !offered {
        return Err(DriverError::Auth("no supported SASL mechanism".to_owned()));
    }
    let mut scram = sasl::ScramSha256::new(password(config)?, sasl::ChannelBinding::unsupported());
    frontend::sasl_initial_response(sasl::SCRAM_SHA_256, scram.message(), wire.out())?;
    wire.flush().await?;
    let body = match wire.next().await? {
        Message::AuthenticationSaslContinue(body) => body,
        other => return Err(rejected(&other)),
    };
    scram.update(body.data()).map_err(auth)?;
    frontend::sasl_response(scram.message(), wire.out())?;
    wire.flush().await?;
    let body = match wire.next().await? {
        Message::AuthenticationSaslFinal(body) => body,
        other => return Err(rejected(&other)),
    };
    scram
        .finish(body.data())
        .map_err(|_| DriverError::Auth("server signature mismatch".to_owned()))
}

/// The server answers a wrong SCRAM proof with `ErrorResponse`, not a SASL message.
fn rejected(message: &Message) -> DriverError {
    match message {
        Message::ErrorResponse(body) => DriverError::Auth(server_error(body).to_string()),
        _ => unexpected("SCRAM exchange"),
    }
}

#[expect(clippy::needless_pass_by_value, reason = "map_err adapter")]
fn auth(error: std::io::Error) -> DriverError {
    DriverError::Auth(error.to_string())
}

fn password(config: &PgConfig) -> Result<&[u8], DriverError> {
    config
        .password
        .as_deref()
        .map(str::as_bytes)
        .ok_or_else(|| DriverError::Auth("server requires a password".to_owned()))
}
