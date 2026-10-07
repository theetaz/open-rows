// SPDX-License-Identifier: Apache-2.0
//! Startup and authentication: cleartext, MD5 and SCRAM-SHA-256 (no channel binding).
use crate::wire::{Wire, server_error, unexpected};
use fallible_iterator::FallibleIterator;
use openrows_driver_api::DriverError;
use postgres_protocol::authentication::{md5_hash, sasl};
use postgres_protocol::message::backend::{AuthenticationSaslBody, Message};
use postgres_protocol::message::frontend;

/// Plain-TCP connection settings for the spike (no TLS, SSH or URL parsing).
pub struct PgConfig {
    /// Host name or IP address.
    pub host: String,
    /// TCP port.
    pub port: u16,
    /// Role name.
    pub user: String,
    /// Password, if the server asks for one.
    pub password: Option<String>,
    /// Database name.
    pub database: String,
}

pub(crate) async fn startup(wire: &mut Wire, config: &PgConfig) -> Result<(), DriverError> {
    let parameters = [
        ("user", config.user.as_str()),
        ("database", config.database.as_str()),
        ("client_encoding", "UTF8"),
        ("application_name", "openrows"),
    ];
    frontend::startup_message(parameters, wire.out())?;
    wire.flush().await?;
    authenticate(wire, config).await?;
    loop {
        match wire.next().await? {
            Message::ReadyForQuery(_) => return Ok(()),
            Message::ParameterStatus(_)
            | Message::BackendKeyData(_)
            | Message::NoticeResponse(_) => {}
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
    let Message::AuthenticationSaslContinue(body) = wire.next().await? else {
        return Err(unexpected("SCRAM exchange"));
    };
    scram.update(body.data()).map_err(auth)?;
    frontend::sasl_response(scram.message(), wire.out())?;
    wire.flush().await?;
    let Message::AuthenticationSaslFinal(body) = wire.next().await? else {
        return Err(unexpected("SCRAM exchange"));
    };
    scram
        .finish(body.data())
        .map_err(|_| DriverError::Auth("server signature mismatch".to_owned()))
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
