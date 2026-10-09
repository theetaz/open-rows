// SPDX-License-Identifier: Apache-2.0
//! SOCKS5 client handshake: RFC 1928 `CONNECT` with "no authentication" or
//! RFC 1929 username/password. Host names go to the proxy unresolved.

use std::net::IpAddr;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use zeroize::Zeroizing;

use super::{ProxyError, ProxySpec};

const VERSION: u8 = 5;
const NO_AUTH: u8 = 0x00;
const USER_PASSWORD: u8 = 0x02;
const NO_ACCEPTABLE: u8 = 0xFF;
const CONNECT: u8 = 0x01;
const IPV4: u8 = 0x01;
const DOMAIN: u8 = 0x03;
const IPV6: u8 = 0x04;

/// Run the handshake on `stream` so that it carries `host:port` afterwards.
pub(super) async fn negotiate<S>(
    stream: &mut S,
    proxy: &ProxySpec,
    host: &str,
    port: u16,
) -> Result<(), ProxyError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let request = connect_request(host, port)?;
    let mut io = Exchange {
        stream,
        proxy: proxy.describe(),
    };
    let methods: &[u8] = if proxy.auth.is_some() {
        &[VERSION, 2, NO_AUTH, USER_PASSWORD]
    } else {
        &[VERSION, 1, NO_AUTH]
    };
    io.send(methods).await?;
    let [version, method] = io.read_array().await?;
    if version != VERSION {
        return Err(io.protocol(format!("version {version}, expected 5")));
    }
    match method {
        NO_AUTH => {}
        USER_PASSWORD => login(&mut io, proxy).await?,
        NO_ACCEPTABLE if proxy.auth.is_none() => {
            return Err(ProxyError::AuthRequired { proxy: io.proxy });
        }
        other => return Err(io.protocol(format!("unsupported method {other:#04x}"))),
    }
    io.send(&request).await?;
    read_reply(&mut io, &format!("{host}:{port}")).await
}

/// `VER CMD RSV ATYP DST.ADDR DST.PORT`: IP literals as addresses, names
/// as `DOMAIN` so the proxy resolves them (remote DNS).
pub(super) fn connect_request(host: &str, port: u16) -> Result<Vec<u8>, ProxyError> {
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    let mut request = vec![VERSION, CONNECT, 0];
    match bare.parse::<IpAddr>() {
        Ok(IpAddr::V4(address)) => {
            request.push(IPV4);
            request.extend(address.octets());
        }
        Ok(IpAddr::V6(address)) => {
            request.push(IPV6);
            request.extend(address.octets());
        }
        Err(_) => {
            let length = u8::try_from(host.len())
                .ok()
                .filter(|length| *length > 0)
                .ok_or_else(|| ProxyError::Invalid {
                    reason: "SOCKS5 host names must be 1–255 bytes".to_owned(),
                })?;
            request.push(DOMAIN);
            request.push(length);
            request.extend(host.as_bytes());
        }
    }
    request.extend(port.to_be_bytes());
    Ok(request)
}

/// RFC 1929 sub-negotiation.
async fn login<S>(io: &mut Exchange<'_, S>, proxy: &ProxySpec) -> Result<(), ProxyError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let Some(auth) = &proxy.auth else {
        return Err(io.protocol("asked for a password that was not offered".to_owned()));
    };
    let field = |value: &[u8]| {
        u8::try_from(value.len()).map_err(|_| ProxyError::Invalid {
            reason: "SOCKS5 user names and passwords must be at most 255 bytes".to_owned(),
        })
    };
    let user = auth.user.as_bytes();
    let password = auth.password.expose().as_bytes();
    let mut message = Zeroizing::new(vec![1, field(user)?]);
    message.extend(user);
    message.push(field(password)?);
    message.extend(password);
    io.send(&message).await?;
    let [_, status] = io.read_array().await?;
    if status != 0 {
        return Err(ProxyError::AuthFailed {
            proxy: io.proxy.clone(),
        });
    }
    Ok(())
}

/// `VER REP RSV ATYP BND.ADDR BND.PORT`; the bound address is skipped.
async fn read_reply<S>(io: &mut Exchange<'_, S>, destination: &str) -> Result<(), ProxyError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let [version, reply, _, address_type] = io.read_array().await?;
    if version != VERSION {
        return Err(io.protocol(format!("version {version}, expected 5")));
    }
    if reply != 0 {
        return Err(ProxyError::Refused {
            proxy: io.proxy.clone(),
            destination: destination.to_owned(),
            reason: reply_text(reply).to_owned(),
        });
    }
    let address = match address_type {
        IPV4 => 4,
        IPV6 => 16,
        DOMAIN => usize::from(io.read_array::<1>().await?[0]),
        other => return Err(io.protocol(format!("address type {other:#04x}"))),
    };
    let mut rest = vec![0; address + 2];
    io.read_exact(&mut rest).await
}

/// RFC 1928 §6 reply codes.
pub(super) fn reply_text(code: u8) -> &'static str {
    match code {
        1 => "general SOCKS server failure",
        2 => "connection not allowed by ruleset",
        3 => "network unreachable",
        4 => "host unreachable",
        5 => "connection refused",
        6 => "TTL expired",
        7 => "command not supported",
        8 => "address type not supported",
        _ => "unknown SOCKS error",
    }
}

/// The stream plus the proxy's name for error messages.
struct Exchange<'a, S> {
    stream: &'a mut S,
    proxy: String,
}

impl<S: AsyncRead + AsyncWrite + Unpin> Exchange<'_, S> {
    async fn send(&mut self, bytes: &[u8]) -> Result<(), ProxyError> {
        self.stream
            .write_all(bytes)
            .await
            .map_err(|source| self.io(source))
    }

    async fn read_exact(&mut self, buffer: &mut [u8]) -> Result<(), ProxyError> {
        self.stream
            .read_exact(buffer)
            .await
            .map(drop)
            .map_err(|source| self.io(source))
    }

    async fn read_array<const N: usize>(&mut self) -> Result<[u8; N], ProxyError> {
        let mut buffer = [0; N];
        self.read_exact(&mut buffer).await?;
        Ok(buffer)
    }

    fn io(&self, source: std::io::Error) -> ProxyError {
        ProxyError::Io {
            proxy: self.proxy.clone(),
            source,
        }
    }

    fn protocol(&self, reason: String) -> ProxyError {
        ProxyError::Protocol {
            proxy: self.proxy.clone(),
            reason,
        }
    }
}
