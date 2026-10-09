// SPDX-License-Identifier: Apache-2.0
//! Proxy hops in front of a route (docs/24 §2): SOCKS5 (RFC 1928, username
//! and password per RFC 1929, host names resolved by the proxy) and HTTP
//! `CONNECT` with optional Basic authentication.
//!
//! A proxy is always the first hop: it opens the TCP stream that either the
//! database protocol ([`ProxyDialer`]) or the first SSH hop runs over.

mod dialer;
mod error;
mod http;
mod socks5;

use std::{fmt, future::Future, io, net::SocketAddr, time::Duration};

use tokio::net::{TcpStream, lookup_host};

pub use dialer::ProxyDialer;
pub use error::ProxyError;

use crate::{SecretString, Stage, Timeouts, configure_tcp};

/// Proxy protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProxyKind {
    /// SOCKS5; the proxy resolves host names (remote DNS).
    Socks5,
    /// HTTP `CONNECT` tunnel.
    HttpConnect,
}

impl fmt::Display for ProxyKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Socks5 => "SOCKS5",
            Self::HttpConnect => "HTTP",
        })
    }
}

/// Proxy login. `Debug` redacts the password.
#[derive(Clone, Debug)]
pub struct ProxyAuth {
    /// User name.
    pub user: String,
    /// Password from the keychain; empty when the proxy wants only a name.
    pub password: SecretString,
}

/// A proxy server and how to log in to it.
#[derive(Clone, Debug)]
pub struct ProxySpec {
    /// Protocol.
    pub kind: ProxyKind,
    /// Proxy host name or IP address.
    pub host: String,
    /// Proxy port.
    pub port: u16,
    /// Login; `None` for anonymous proxies.
    pub auth: Option<ProxyAuth>,
}

impl ProxySpec {
    /// `SOCKS5 proxy.example.com:1080`, for routes and messages.
    #[must_use]
    pub fn describe(&self) -> String {
        format!("{} proxy {}:{}", self.kind, self.host, self.port)
    }

    /// Reach the proxy and ask it for a stream to `host:port`; `host` is
    /// sent as a name (the proxy resolves it) unless it is an IP literal.
    ///
    /// # Errors
    /// [`ProxyError`] naming the stage: resolving or reaching the proxy, its
    /// authentication, or the proxy refusing the destination.
    pub async fn connect(
        &self,
        host: &str,
        port: u16,
        timeouts: &Timeouts,
    ) -> Result<TcpStream, ProxyError> {
        let mut stream = self.reach(timeouts).await?;
        let negotiate = async {
            match self.kind {
                ProxyKind::Socks5 => socks5::negotiate(&mut stream, self, host, port).await,
                ProxyKind::HttpConnect => http::negotiate(&mut stream, self, host, port).await,
            }
        };
        limit(Stage::Proxy, timeouts.tcp_connect, negotiate).await?;
        Ok(stream)
    }

    /// DNS and TCP to the proxy itself.
    async fn reach(&self, timeouts: &Timeouts) -> Result<TcpStream, ProxyError> {
        let resolve = async {
            let addresses: Vec<SocketAddr> = lookup_host((self.host.as_str(), self.port))
                .await
                .map_err(|source| self.unreachable(source))?
                .collect();
            if addresses.is_empty() {
                return Err(self.unreachable(io::ErrorKind::NotFound.into()));
            }
            Ok(addresses)
        };
        let addresses = limit(Stage::Resolve, timeouts.dns, resolve).await?;
        let connect = async {
            let mut last = io::Error::from(io::ErrorKind::NotFound);
            for address in &addresses {
                match TcpStream::connect(address).await {
                    Ok(stream) => return Ok(stream),
                    Err(error) => last = error,
                }
            }
            Err(self.unreachable(last))
        };
        let stream = limit(Stage::TcpConnect, timeouts.tcp_connect, connect).await?;
        configure_tcp(&stream).map_err(|source| self.unreachable(source))?;
        Ok(stream)
    }

    fn unreachable(&self, source: io::Error) -> ProxyError {
        ProxyError::Unreachable {
            proxy: self.describe(),
            source,
        }
    }
}

/// Run one proxy stage within `budget`.
async fn limit<T>(
    stage: Stage,
    budget: Duration,
    work: impl Future<Output = Result<T, ProxyError>>,
) -> Result<T, ProxyError> {
    tokio::time::timeout(budget, work)
        .await
        .map_err(|_| ProxyError::Timeout {
            stage,
            after: budget,
        })?
}

#[cfg(test)]
mod tests;
