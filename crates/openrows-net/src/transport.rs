// SPDX-License-Identifier: Apache-2.0
//! Byte streams handed to drivers and the dialers that open them for a route
//! (docs/24 §1: drivers ask the network layer for a stream, they never open
//! sockets for routed connections themselves).

use std::{
    fmt, io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::{BoxFuture, SharedTunnel, SshError, SshTunnel, Stage, TunnelStream};

/// A connected, bidirectional byte stream.
pub trait NetStream: AsyncRead + AsyncWrite + Unpin + Send {}

impl<T: AsyncRead + AsyncWrite + Unpin + Send> NetStream for T {}

/// An owned stream of any transport.
pub type BoxStream = Box<dyn NetStream>;

/// Opens a fresh stream to one fixed destination: one call per database
/// connection, metadata connection or cancel request.
pub trait Dialer: Send + Sync {
    /// Open a new stream to the destination.
    ///
    /// # Errors
    /// An I/O error whose message names the hop that failed.
    fn dial(&self) -> BoxFuture<'_, io::Result<BoxStream>>;

    /// Human-readable route, e.g. `SSH bastion.example.com → db.internal:5432`.
    fn describe(&self) -> String;

    /// True when streams reach a Unix socket on the far side, where TLS
    /// does not apply.
    fn is_socket(&self) -> bool {
        false
    }
}

/// What a tunnel stream connects to on the last hop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Destination {
    /// `host:port` as seen from the last hop (`direct-tcpip`).
    Tcp {
        /// Host name or address.
        host: String,
        /// Port.
        port: u16,
    },
    /// A Unix socket file on the last hop (`direct-streamlocal@openssh.com`).
    Socket(String),
}

impl fmt::Display for Destination {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tcp { host, port } => write!(formatter, "{host}:{port}"),
            Self::Socket(path) => formatter.write_str(path),
        }
    }
}

/// Opens forwarded channels to a [`Destination`] through a profile's
/// [`SharedTunnel`], reconnecting the tunnel once when its transport dropped.
///
/// Every stream keeps its tunnel alive, so the SSH sessions close only after
/// the last database connection using them is gone.
#[derive(Clone)]
pub struct TunnelDialer {
    shared: Arc<SharedTunnel>,
    destination: Destination,
}

impl TunnelDialer {
    /// Reach `destination` as seen from the tunnel's last hop.
    #[must_use]
    pub fn new(shared: Arc<SharedTunnel>, destination: Destination) -> Self {
        Self {
            shared,
            destination,
        }
    }

    /// The shared tunnel.
    #[must_use]
    pub fn shared(&self) -> &Arc<SharedTunnel> {
        &self.shared
    }

    async fn open(&self, tunnel: &Arc<SshTunnel>) -> Result<BoxStream, SshError> {
        let stream = match &self.destination {
            Destination::Tcp { host, port } => tunnel.open_stream(host, *port).await?,
            Destination::Socket(path) => tunnel.open_socket(path).await?,
        };
        Ok(Box::new(Tunnelled {
            stream,
            _tunnel: Arc::clone(tunnel),
        }))
    }

    /// Open a stream; when the tunnel turns out to be dead, reconnect it
    /// once and retry, with a clear error if that fails.
    async fn dial_with_reconnect(&self) -> io::Result<BoxStream> {
        let tunnel = self
            .shared
            .tunnel()
            .await
            .map_err(|error| reopen_failed(&error))?;
        match self.open(&tunnel).await {
            Ok(stream) => Ok(stream),
            Err(error) if dropped(&tunnel, &error) => {
                self.shared.discard(&tunnel);
                let fresh = self
                    .shared
                    .tunnel()
                    .await
                    .map_err(|error| reopen_failed(&error))?;
                self.open(&fresh).await.map_err(io::Error::other)
            }
            Err(error) => Err(io::Error::other(error)),
        }
    }
}

/// The tunnel's transport is gone or stopped answering (half-open after
/// sleep or a network change), as opposed to the destination refusing.
fn dropped(tunnel: &SshTunnel, error: &SshError) -> bool {
    tunnel.is_closed()
        || matches!(
            error,
            SshError::Timeout {
                stage: Stage::ChannelOpen,
                ..
            } | SshError::Protocol(_)
        )
}

fn reopen_failed(error: &SshError) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotConnected,
        format!("the SSH tunnel is closed and could not be re-established: {error}"),
    )
}

impl Dialer for TunnelDialer {
    fn dial(&self) -> BoxFuture<'_, io::Result<BoxStream>> {
        Box::pin(self.dial_with_reconnect())
    }

    fn describe(&self) -> String {
        format!(
            "SSH {} → {}",
            self.shared.hosts().join(" → "),
            self.destination
        )
    }

    fn is_socket(&self) -> bool {
        matches!(self.destination, Destination::Socket(_))
    }
}

impl fmt::Debug for TunnelDialer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.describe())
    }
}

/// A forwarded stream that keeps its tunnel open.
struct Tunnelled {
    stream: TunnelStream,
    _tunnel: Arc<SshTunnel>,
}

impl AsyncRead for Tunnelled {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(context, buffer)
    }
}

impl AsyncWrite for Tunnelled {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(context, data)
    }

    fn poll_flush(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(context)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(context)
    }
}
