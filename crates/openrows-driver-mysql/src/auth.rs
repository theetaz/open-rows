// SPDX-License-Identifier: Apache-2.0
//! The login exchange after the handshake response: auth switches, more
//! data (the `caching_sha2_password` fast path, full authentication over
//! TLS, or the RSA key exchange in plaintext) until `OK` or `ERR`. The
//! cryptography is `mysql_common`'s sans-IO plugins.
use mysql_common::auth::plugins::{AuthProc, ChallengeResponsePlugin, Context, Response};
use mysql_common::constants::CapabilityFlags;
use mysql_common::io::ParseBuf;
use mysql_common::packets::{AuthPlugin, AuthSwitchRequest};
use openrows_driver_api::DriverError;

use crate::error::{err_packet, unexpected};
use crate::packet::Packets;

/// Scramble length since `MySQL` 4.1.
const SCRAMBLE: usize = 20;

/// What the plugins may know about the login.
#[derive(Clone, Copy)]
pub(crate) struct Secrets<'a> {
    pub(crate) password: &'a [u8],
    /// TLS or a local socket: the cleartext password may be sent.
    pub(crate) secure: bool,
    pub(crate) scramble: &'a [u8],
}

impl Context for Secrets<'_> {
    fn pass(&self) -> &[u8] {
        self.password
    }

    fn is_ipc_transport(&self) -> bool {
        false
    }

    fn is_tls_transport(&self) -> bool {
        self.secure
    }

    fn scramble(&self) -> &[u8] {
        self.scramble
    }

    fn server_key_pem(&self) -> Option<&[u8]> {
        None
    }
}

/// An auth plugin in progress with the nonce it was given.
pub(crate) struct Exchange {
    plugin: AuthProc,
    nonce: Vec<u8>,
}

impl Exchange {
    /// Start `plugin` with the handshake `nonce`; returns the first response.
    pub(crate) fn start(
        plugin: &AuthPlugin<'_>,
        nonce: &[u8],
        secrets: Secrets<'_>,
    ) -> Result<(Self, Option<Vec<u8>>), DriverError> {
        if matches!(plugin, AuthPlugin::MysqlClearPassword) && !secrets.secure {
            return Err(DriverError::Auth(
                "the server asks for the cleartext password over an unencrypted connection; \
                 enable TLS to use this account"
                    .to_owned(),
            ));
        }
        let plugin =
            AuthProc::init(plugin).map_err(|error| DriverError::Auth(error.to_string()))?;
        let mut exchange = Self {
            plugin,
            nonce: nonce.get(..SCRAMBLE).unwrap_or(nonce).to_vec(),
        };
        let first = exchange.step(secrets, &exchange.nonce.clone())?;
        Ok((exchange, first))
    }

    fn step(
        &mut self,
        secrets: Secrets<'_>,
        challenge: &[u8],
    ) -> Result<Option<Vec<u8>>, DriverError> {
        let secrets = Secrets {
            scramble: &self.nonce,
            ..secrets
        };
        let response: Response = self
            .plugin
            .run(secrets, challenge)
            .map_err(|error| DriverError::Auth(format!("authentication failed: {error}")))?;
        Ok(response.data().map(<[u8]>::to_vec))
    }

    /// Continue until the server accepts (`OK`) or refuses (`ERR`).
    pub(crate) async fn finish(
        mut self,
        packets: &mut Packets,
        secrets: Secrets<'_>,
        capabilities: CapabilityFlags,
    ) -> Result<(), DriverError> {
        loop {
            let packet = packets.read().await?;
            let reply = match packet.first() {
                Some(0x00) => return Ok(()),
                Some(0xff) => {
                    return Err(err_packet(&packet, capabilities)
                        .unwrap_or_else(|| unexpected("during login", &packet)));
                }
                Some(0xfe) => self.switch(&packet, secrets)?,
                Some(0x01) => self.step(secrets, packet.get(1..).unwrap_or_default())?,
                _ => return Err(unexpected("during login", &packet)),
            };
            if let Some(data) = reply {
                packets.write(&data).await?;
            }
        }
    }

    /// `AuthSwitchRequest`: restart with the plugin and nonce the server names.
    fn switch(
        &mut self,
        packet: &[u8],
        secrets: Secrets<'_>,
    ) -> Result<Option<Vec<u8>>, DriverError> {
        if packet.len() == 1 {
            return Err(DriverError::Auth(
                "the server asked for the insecure pre-4.1 password scheme".to_owned(),
            ));
        }
        let request = ParseBuf(packet)
            .parse::<AuthSwitchRequest<'_>>(())
            .map_err(|error| DriverError::Protocol(format!("auth switch: {error}")))?;
        let data = request.plugin_data();
        let nonce = data.strip_suffix(&[0]).unwrap_or(data);
        let (exchange, first) = Self::start(&request.auth_plugin(), nonce, secrets)?;
        *self = exchange;
        Ok(Some(first.unwrap_or_default()))
    }
}
