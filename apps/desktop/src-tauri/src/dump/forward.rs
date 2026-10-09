// SPDX-License-Identifier: Apache-2.0
//! The loopback end of a profile's SSH tunnel or proxy for the command-line
//! tools, which need a socket address (docs/12 §4.2): a listener on
//! `127.0.0.1` with an ephemeral port that accepts only the connections the
//! tool is expected to open, each piped through the profile's dialer. It
//! closes when the job ends.
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use openrows_net::Dialer;
use tauri::async_runtime::JoinHandle;
use tokio::net::{TcpListener, TcpStream};

/// A listening forward; dropping it stops accepting.
pub(super) struct Forward {
    port: u16,
    accept: JoinHandle<()>,
}

impl Forward {
    /// Listen on `127.0.0.1` and forward at most `connections` connections
    /// through `dialer`.
    ///
    /// # Errors
    /// No loopback port could be bound.
    pub(super) async fn open(dialer: Arc<dyn Dialer>, connections: usize) -> std::io::Result<Self> {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await?;
        let port = listener.local_addr()?.port();
        let accept = tauri::async_runtime::spawn(accept(listener, dialer, connections));
        Ok(Self { port, accept })
    }

    /// The local port the tool connects to.
    pub(super) fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for Forward {
    fn drop(&mut self) {
        self.accept.abort();
    }
}

/// Accept `connections` connections, then close the listener.
async fn accept(listener: TcpListener, dialer: Arc<dyn Dialer>, connections: usize) {
    for _ in 0..connections {
        let Ok((socket, _)) = listener.accept().await else {
            return;
        };
        tauri::async_runtime::spawn(pipe(socket, Arc::clone(&dialer)));
    }
}

/// Copy bytes both ways until either side closes; a failed dial closes the
/// tool's connection, which then reports the error.
async fn pipe(mut socket: TcpStream, dialer: Arc<dyn Dialer>) {
    let Ok(mut remote) = dialer.dial().await else {
        return;
    };
    // Either side closing ends the copy; errors only mean the tool or server went away.
    let _ = tokio::io::copy_bidirectional(&mut socket, &mut remote).await;
}

#[cfg(test)]
mod tests {
    use openrows_net::{BoxFuture, BoxStream};
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    use super::*;

    /// Dials a local echo server, like a tunnel to a database.
    struct Echo(SocketAddr);

    impl Dialer for Echo {
        fn dial(&self) -> BoxFuture<'_, std::io::Result<BoxStream>> {
            Box::pin(async move {
                let stream = TcpStream::connect(self.0).await?;
                Ok(Box::new(stream) as BoxStream)
            })
        }

        fn describe(&self) -> String {
            "echo".to_owned()
        }
    }

    async fn echo_server() -> std::io::Result<SocketAddr> {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await?;
        let address = listener.local_addr()?;
        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                tokio::spawn(echo(socket));
            }
        });
        Ok(address)
    }

    async fn echo(mut socket: TcpStream) {
        let (mut read, mut write) = socket.split();
        let _ = tokio::io::copy(&mut read, &mut write).await;
    }

    async fn round_trip(port: u16) -> std::io::Result<Vec<u8>> {
        let mut client = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).await?;
        client.write_all(b"ping").await?;
        let mut reply = vec![0; 4];
        client.read_exact(&mut reply).await?;
        Ok(reply)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn forwards_only_the_expected_connections() -> std::io::Result<()> {
        let forward = Forward::open(Arc::new(Echo(echo_server().await?)), 1).await?;
        assert_eq!(round_trip(forward.port()).await?, b"ping");
        // The listener closed after its one connection.
        assert!(round_trip(forward.port()).await.is_err());
        Ok(())
    }
}
