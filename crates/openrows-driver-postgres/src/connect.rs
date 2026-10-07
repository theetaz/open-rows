// SPDX-License-Identifier: Apache-2.0
//! Connection setup: DNS, TCP or Unix socket, startup, server info and startup
//! commands, each network stage under its own timeout (docs/04 §6, docs/14 §4).
use crate::cancel::PgCancel;
use crate::info::{self, ServerInfo};
use crate::options::{ConnectOptions, ConnectTimeouts};
use crate::simple;
use crate::startup::{PgConfig, startup};
use crate::wire::{Io, Wire};
use openrows_driver_api::DriverError;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use tokio::net::TcpStream;

/// Where the server listens: a TCP endpoint or a Unix socket file.
#[derive(Clone, Debug)]
pub(crate) enum Target {
    Tcp { host: String, port: u16 },
    Unix(PathBuf),
}

impl Target {
    /// libpq rule: a host starting with `/` is a socket directory.
    pub(crate) fn of(host: &str, port: u16) -> Self {
        if host.starts_with('/') {
            Self::Unix(PathBuf::from(format!("{host}/.s.PGSQL.{port}")))
        } else {
            Self::Tcp {
                host: host.to_owned(),
                port,
            }
        }
    }

    pub(crate) async fn open(
        &self,
        timeouts: &ConnectTimeouts,
    ) -> Result<Box<dyn Io>, DriverError> {
        match self {
            Self::Tcp { host, port } => tcp(host, *port, timeouts).await,
            Self::Unix(path) => unix(path.clone(), timeouts.tcp).await,
        }
    }
}

/// A started, authenticated connection ready for statements.
pub(crate) struct Established {
    pub(crate) wire: Wire,
    pub(crate) cancel: Option<PgCancel>,
    pub(crate) info: ServerInfo,
}

pub(crate) async fn establish(
    config: &PgConfig,
    options: &ConnectOptions,
) -> Result<Established, DriverError> {
    let tls = info::tls_status(options.ssl_mode)?;
    let target = Target::of(&config.host, config.port);
    let mut wire = Wire::new(target.open(&options.timeouts).await?);
    let session = async {
        let started = startup(&mut wire, config, &options.application_name).await?;
        let info = info::fetch(&mut wire, config, started.server_version, tls).await?;
        Ok((started.key, info))
    };
    let (key, info) = within(options.timeouts.auth, "authentication", session).await?;
    for (index, sql) in options.startup_commands.iter().enumerate() {
        simple::query(&mut wire, sql)
            .await
            .map_err(|error| startup_command_error(index, error))?;
    }
    let cancel = key.map(|key| PgCancel::new(target, key));
    Ok(Established { wire, cancel, info })
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
    let resolve = async { Ok(tokio::net::lookup_host((host, port)).await?.collect()) };
    let addresses: Vec<SocketAddr> = within(timeouts.dns, "resolving the host", resolve).await?;
    let stream = within(timeouts.tcp, "TCP connect", connect_any(&addresses)).await?;
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
mod tests {
    use super::*;

    #[test]
    fn socket_directories_become_unix_targets() {
        let Target::Unix(path) = Target::of("/tmp", 5433) else {
            unreachable!("a leading slash means a socket directory")
        };
        assert_eq!(path, PathBuf::from("/tmp/.s.PGSQL.5433"));
        assert!(matches!(
            Target::of("localhost", 5432),
            Target::Tcp { port: 5432, .. }
        ));
    }

    #[tokio::test]
    async fn a_stage_timeout_names_the_stage() {
        let never = std::future::pending::<Result<(), DriverError>>();
        let error = within(Duration::from_millis(1), "TCP connect", never).await;
        let Err(DriverError::Network(error)) = error else {
            unreachable!("pending future must time out")
        };
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert_eq!(error.to_string(), "TCP connect timed out after 1ms");
    }
}
