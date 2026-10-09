// SPDX-License-Identifier: Apache-2.0
//! Reaching the server: DNS and TCP, a Unix socket file, or a stream through
//! the route (SSH tunnel), each stage under its own timeout.
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use openrows_driver_api::{ConnectStage, ConnectTimeouts, ConnectTrace, DriverError, Endpoint};
use openrows_net::{BoxStream, Dialer, configure_tcp};
use tokio::net::TcpStream;

/// Where the server listens.
#[derive(Clone)]
pub(crate) enum Target {
    Tcp { host: String, port: u16 },
    Unix(PathBuf),
    Via(Arc<dyn Dialer>),
}

impl Target {
    /// A route replaces direct TCP; a host starting with `/` is the server's
    /// socket file (e.g. `/tmp/mysql.sock`).
    pub(crate) fn of(
        endpoint: &Endpoint,
        via: Option<&Arc<dyn Dialer>>,
    ) -> Result<Self, DriverError> {
        let Endpoint::Server { host, port } = endpoint else {
            return Err(DriverError::Unsupported(
                "MySQL connects to a server, not a file".to_owned(),
            ));
        };
        Ok(if let Some(dialer) = via {
            Self::Via(Arc::clone(dialer))
        } else if host.starts_with('/') {
            Self::Unix(PathBuf::from(host))
        } else {
            Self::Tcp {
                host: host.clone(),
                port: *port,
            }
        })
    }

    /// TLS applies to TCP and routed streams, not to local sockets nor to
    /// sockets reached through an SSH tunnel.
    pub(crate) fn tcp_like(&self) -> bool {
        match self {
            Self::Tcp { .. } => true,
            Self::Unix(_) => false,
            Self::Via(dialer) => !dialer.is_socket(),
        }
    }

    /// Open a stream, recording DNS and connect as separate stages for TCP.
    pub(crate) async fn open(
        &self,
        timeouts: &ConnectTimeouts,
        trace: &mut ConnectTrace,
    ) -> Result<BoxStream, DriverError> {
        match self {
            Self::Tcp { host, port } => {
                let addresses = trace
                    .stage(ConnectStage::Resolve, resolve(host, *port, timeouts.dns))
                    .await?;
                let connect = within(timeouts.tcp, "TCP connect", connect_any(&addresses));
                trace.stage(ConnectStage::Connect, connect).await
            }
            Self::Unix(path) => {
                let connect = within(timeouts.tcp, "Unix socket connect", unix(path.clone()));
                trace.stage(ConnectStage::Connect, connect).await
            }
            Self::Via(dialer) => {
                let dial = async { Ok(dialer.dial().await?) };
                let open = within(timeouts.tcp, "opening the tunnel stream", dial);
                trace.stage(ConnectStage::Connect, open).await
            }
        }
    }
}

async fn resolve(host: &str, port: u16, limit: Duration) -> Result<Vec<SocketAddr>, DriverError> {
    let lookup = async {
        let addresses = tokio::net::lookup_host((host, port))
            .await
            .map_err(|error| io::Error::new(error.kind(), format!("resolving {host}: {error}")))?;
        Ok(addresses.collect())
    };
    within(limit, "resolving the host", lookup).await
}

/// Try each resolved address in order; report the last failure.
async fn connect_any(addresses: &[SocketAddr]) -> Result<BoxStream, DriverError> {
    let mut last = io::Error::new(io::ErrorKind::NotFound, "host resolved to no addresses");
    for address in addresses {
        match TcpStream::connect(address).await {
            Ok(stream) => {
                configure_tcp(&stream)?;
                return Ok(Box::new(stream));
            }
            Err(error) => last = io::Error::new(error.kind(), format!("{address}: {error}")),
        }
    }
    Err(last.into())
}

#[cfg(unix)]
async fn unix(path: PathBuf) -> Result<BoxStream, DriverError> {
    let stream = tokio::net::UnixStream::connect(&path)
        .await
        .map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", path.display())))?;
    Ok(Box::new(stream))
}

#[cfg(not(unix))]
async fn unix(_path: PathBuf) -> Result<BoxStream, DriverError> {
    Err(DriverError::Unsupported(
        "Unix-domain sockets on this platform".to_owned(),
    ))
}

/// Run one stage; a timeout becomes a `Network` error of kind `TimedOut`
/// that names the stage.
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
