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

use crate::{BoxFuture, SshTunnel, TunnelStream};

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
}

/// Opens `direct-tcpip` channels to `host:port` through an SSH tunnel.
///
/// Every stream keeps the tunnel alive, so the SSH sessions close only after
/// the last database connection using them is gone.
#[derive(Clone)]
pub struct TunnelDialer {
    tunnel: Arc<SshTunnel>,
    host: String,
    port: u16,
}

impl TunnelDialer {
    /// Reach `host:port` as seen from the tunnel's last hop.
    #[must_use]
    pub fn new(tunnel: Arc<SshTunnel>, host: &str, port: u16) -> Self {
        Self {
            tunnel,
            host: host.to_owned(),
            port,
        }
    }

    /// The shared tunnel.
    #[must_use]
    pub fn tunnel(&self) -> &Arc<SshTunnel> {
        &self.tunnel
    }
}

impl Dialer for TunnelDialer {
    fn dial(&self) -> BoxFuture<'_, io::Result<BoxStream>> {
        Box::pin(async move {
            let stream = self
                .tunnel
                .open_stream(&self.host, self.port)
                .await
                .map_err(io::Error::other)?;
            let stream: BoxStream = Box::new(Tunnelled {
                stream,
                _tunnel: Arc::clone(&self.tunnel),
            });
            Ok(stream)
        })
    }

    fn describe(&self) -> String {
        let hops: Vec<&str> = self
            .tunnel
            .timings()
            .iter()
            .map(|hop| hop.host.as_str())
            .collect();
        format!("SSH {} → {}:{}", hops.join(" → "), self.host, self.port)
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
