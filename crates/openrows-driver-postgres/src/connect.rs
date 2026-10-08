// SPDX-License-Identifier: Apache-2.0
//! Connection setup: DNS and TCP, a Unix socket or a routed stream, TLS
//! negotiation, startup, server info and startup commands, each stage under
//! its own timeout (docs/04 §6, docs/14 §4) and recorded in a [`ConnectTrace`].
use crate::cancel::PgCancel;
use crate::info::{self, ServerInfo};
use crate::options::{ConnectOptions, ConnectTimeouts, SslMode};
use crate::simple;
use crate::startup::{PgConfig, startup};
use crate::tls::{self, Negotiation};
use crate::trace::{ConnectStage, ConnectTrace};
use crate::wire::{Io, Wire};
use openrows_driver_api::DriverError;
use openrows_net::Dialer;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;

/// Where the server listens: a TCP endpoint, a Unix socket file, or the far
/// end of a route such as an SSH tunnel.
#[derive(Clone)]
pub(crate) enum Target {
    Tcp { host: String, port: u16 },
    Unix(PathBuf),
    Via(Arc<dyn Dialer>),
}

impl Target {
    /// libpq rule: a host starting with `/` is a socket directory. A route
    /// replaces direct TCP.
    pub(crate) fn of(host: &str, port: u16, via: Option<&Arc<dyn Dialer>>) -> Self {
        if let Some(dialer) = via {
            Self::Via(Arc::clone(dialer))
        } else if host.starts_with('/') {
            Self::Unix(PathBuf::from(format!("{host}/.s.PGSQL.{port}")))
        } else {
            Self::Tcp {
                host: host.to_owned(),
                port,
            }
        }
    }

    /// True for transports where TLS applies.
    fn tcp_like(&self) -> bool {
        !matches!(self, Self::Unix(_))
    }

    pub(crate) async fn open(
        &self,
        timeouts: &ConnectTimeouts,
    ) -> Result<Box<dyn Io>, DriverError> {
        match self {
            Self::Tcp { host, port } => tcp(host, *port, timeouts).await,
            Self::Unix(path) => unix(path.clone(), timeouts.tcp).await,
            Self::Via(dialer) => {
                let dial = async { Ok(dialer.dial().await?) };
                let stream = within(timeouts.tcp, "opening the tunnel stream", dial).await?;
                Ok(Box::new(stream))
            }
        }
    }
}

/// A started, authenticated connection ready for statements.
pub(crate) struct Established {
    pub(crate) wire: Wire,
    pub(crate) cancel: Option<PgCancel>,
    pub(crate) info: ServerInfo,
}

/// Connect, recording each stage in `trace`. `sslmode=allow` retries with
/// TLS when the server rejects the plaintext session, as libpq does.
pub(crate) async fn establish(
    config: &PgConfig,
    options: &ConnectOptions,
    trace: &mut ConnectTrace,
) -> Result<Established, DriverError> {
    let connector = Connector {
        config,
        options,
        target: Target::of(&config.host, config.port, options.via.as_ref()),
    };
    let first = Negotiation::first(options.ssl_mode, connector.target.tcp_like());
    match connector.attempt(first, trace).await {
        Err(error) if retry_with_tls(options.ssl_mode, first, &error) => {
            trace.restart();
            connector.attempt(Negotiation::Required, trace).await
        }
        result => result,
    }
}

fn retry_with_tls(mode: SslMode, first: Negotiation, error: &DriverError) -> bool {
    mode == SslMode::Allow
        && first == Negotiation::Plaintext
        && matches!(error, DriverError::Auth(_) | DriverError::Server { .. })
}

struct Connector<'a> {
    config: &'a PgConfig,
    options: &'a ConnectOptions,
    target: Target,
}

impl Connector<'_> {
    async fn attempt(
        &self,
        negotiation: Negotiation,
        trace: &mut ConnectTrace,
    ) -> Result<Established, DriverError> {
        let (config, options) = (self.config, self.options);
        let stream = self.open(trace).await?;
        let negotiate = tls::negotiate(stream, negotiation, options, &config.host);
        let (stream, status) = if negotiation.asks() {
            let limited = within(options.timeouts.tls, "TLS negotiation", negotiate);
            trace.stage(ConnectStage::Tls, limited).await?
        } else {
            negotiate.await?
        };
        trace.tls = Some(status.clone());
        let mut wire = Wire::new(stream);
        let login = within(
            options.timeouts.auth,
            "authentication",
            startup(&mut wire, config, &options.application_name),
        );
        let started = trace
            .stage(ConnectStage::Authenticate, async {
                login.await.map_err(tls::post_handshake)
            })
            .await?;
        let fetch = info::fetch(&mut wire, config, started.server_version, status);
        let info = within(options.timeouts.auth, "reading the server version", fetch);
        let info = trace.stage(ConnectStage::ServerInfo, info).await?;
        let commands = run_startup_commands(&mut wire, &options.startup_commands);
        trace.stage(ConnectStage::StartupCommands, commands).await?;
        let cancel = started
            .key
            .map(|key| PgCancel::new(self.target.clone(), key));
        Ok(Established { wire, cancel, info })
    }

    /// Open the transport; direct TCP records DNS and TCP as separate stages.
    async fn open(&self, trace: &mut ConnectTrace) -> Result<Box<dyn Io>, DriverError> {
        let timeouts = &self.options.timeouts;
        let Target::Tcp { host, port } = &self.target else {
            let open = self.target.open(timeouts);
            return trace.stage(ConnectStage::Connect, open).await;
        };
        let addresses = trace
            .stage(ConnectStage::Resolve, resolve(host, *port, timeouts))
            .await?;
        let connect = connect_tcp(&addresses, timeouts);
        trace.stage(ConnectStage::Connect, connect).await
    }
}

async fn run_startup_commands(wire: &mut Wire, commands: &[String]) -> Result<(), DriverError> {
    for (index, sql) in commands.iter().enumerate() {
        simple::query(wire, sql)
            .await
            .map_err(|error| startup_command_error(index, error))?;
    }
    Ok(())
}

fn startup_command_error(index: usize, error: DriverError) -> DriverError {
    match error {
        DriverError::Server { code, message } => DriverError::Server {
            code,
            message: format!("startup command {}: {message}", index + 1),
        },
        other => other,
    }
}

async fn tcp(
    host: &str,
    port: u16,
    timeouts: &ConnectTimeouts,
) -> Result<Box<dyn Io>, DriverError> {
    let addresses = resolve(host, port, timeouts).await?;
    connect_tcp(&addresses, timeouts).await
}

async fn resolve(
    host: &str,
    port: u16,
    timeouts: &ConnectTimeouts,
) -> Result<Vec<SocketAddr>, DriverError> {
    let lookup = async {
        let addresses = tokio::net::lookup_host((host, port))
            .await
            .map_err(|error| io::Error::new(error.kind(), format!("resolving {host}: {error}")))?;
        Ok(addresses.collect())
    };
    within(timeouts.dns, "resolving the host", lookup).await
}

async fn connect_tcp(
    addresses: &[SocketAddr],
    timeouts: &ConnectTimeouts,
) -> Result<Box<dyn Io>, DriverError> {
    let stream = within(timeouts.tcp, "TCP connect", connect_any(addresses)).await?;
    stream.set_nodelay(true)?;
    Ok(Box::new(stream))
}

/// Try each resolved address in order; report the last failure.
async fn connect_any(addresses: &[SocketAddr]) -> Result<TcpStream, DriverError> {
    let mut last = io::Error::new(io::ErrorKind::NotFound, "host resolved to no addresses");
    for address in addresses {
        match TcpStream::connect(address).await {
            Ok(stream) => return Ok(stream),
            Err(error) => last = io::Error::new(error.kind(), format!("{address}: {error}")),
        }
    }
    Err(last.into())
}

#[cfg(unix)]
async fn unix(path: PathBuf, limit: Duration) -> Result<Box<dyn Io>, DriverError> {
    let connect = async {
        tokio::net::UnixStream::connect(&path)
            .await
            .map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", path.display())))
    };
    let stream = within(limit, "Unix socket connect", async { Ok(connect.await?) }).await?;
    Ok(Box::new(stream))
}

#[cfg(not(unix))]
async fn unix(_path: PathBuf, _limit: Duration) -> Result<Box<dyn Io>, DriverError> {
    Err(DriverError::Unsupported(
        "Unix-domain sockets on this platform".to_owned(),
    ))
}

/// Run one connect stage; a timeout becomes a `Network` error of kind `TimedOut`
/// that names the stage, so diagnostics can say which step hung.
pub(crate) async fn within<T>(
    limit: Duration,
    stage: &str,
    future: impl Future<Output = Result<T, DriverError>>,
) -> Result<T, DriverError> {
    tokio::time::timeout(limit, future).await.map_err(|_| {
        let message = format!("{stage} timed out after {limit:?}");
        DriverError::Network(io::Error::new(io::ErrorKind::TimedOut, message))
    })?
}

#[cfg(test)]
mod tests;
