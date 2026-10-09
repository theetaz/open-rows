// SPDX-License-Identifier: Apache-2.0
//! A toxiproxy-style TCP relay for resilience tests: clients connect to the
//! link, which forwards to the real server until a test cuts every
//! connection (server-side kill), black-holes traffic (half-open link after
//! sleep or a network change) or refuses new connections (server down).

use std::{
    io,
    net::{Ipv4Addr, SocketAddr},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        TcpListener, TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
    task::JoinHandle,
};

/// Bytes moved per read.
const CHUNK: usize = 16 * 1024;

#[derive(Default)]
struct Faults {
    blackhole: AtomicBool,
    refuse: AtomicBool,
    pumps: Mutex<Vec<JoinHandle<()>>>,
}

impl Faults {
    fn pumps(&self) -> std::sync::MutexGuard<'_, Vec<JoinHandle<()>>> {
        self.pumps.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A relay on `127.0.0.1` in front of `upstream`; stops when dropped.
pub struct FaultyLink {
    /// Listening port; point clients here instead of at the server.
    pub port: u16,
    faults: Arc<Faults>,
    task: JoinHandle<()>,
}

impl FaultyLink {
    /// Listen on an ephemeral loopback port and relay to `upstream`.
    ///
    /// # Errors
    /// The listener cannot be bound.
    pub async fn start(upstream: SocketAddr) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        let faults = Arc::new(Faults::default());
        let shared = Arc::clone(&faults);
        let task = tokio::spawn(async move {
            while let Ok((client, _)) = listener.accept().await {
                accept(client, upstream, &shared).await;
            }
        });
        Ok(Self { port, faults, task })
    }

    /// Close every relayed connection at once, like a server-side kill.
    pub fn cut(&self) {
        for pump in self.faults.pumps().drain(..) {
            pump.abort();
        }
    }

    /// Silently drop all traffic in both directions while keeping the
    /// sockets open (`true`), or forward it again (`false`).
    pub fn set_blackhole(&self, on: bool) {
        self.faults.blackhole.store(on, Ordering::SeqCst);
    }

    /// Close new connections at once, as a stopped server's host does (`true`).
    pub fn set_refusing(&self, on: bool) {
        self.faults.refuse.store(on, Ordering::SeqCst);
    }
}

impl Drop for FaultyLink {
    fn drop(&mut self) {
        self.task.abort();
        self.cut();
    }
}

/// Relay a new client, or close it at once while refusing.
async fn accept(client: TcpStream, upstream: SocketAddr, faults: &Arc<Faults>) {
    if faults.refuse.load(Ordering::SeqCst) {
        return;
    }
    // A server that cannot be reached only fails the test's own client.
    if let Ok(server) = TcpStream::connect(upstream).await {
        relay(client, server, faults);
    }
}

fn relay(client: TcpStream, server: TcpStream, faults: &Arc<Faults>) {
    let (client_read, client_write) = client.into_split();
    let (server_read, server_write) = server.into_split();
    let upstream = tokio::spawn(pump(client_read, server_write, Arc::clone(faults)));
    let downstream = tokio::spawn(pump(server_read, client_write, Arc::clone(faults)));
    faults.pumps().extend([upstream, downstream]);
}

/// Copy one direction, discarding bytes while the link is black-holed.
async fn pump(mut from: OwnedReadHalf, mut to: OwnedWriteHalf, faults: Arc<Faults>) {
    let mut buffer = vec![0; CHUNK];
    while let Ok(read) = from.read(&mut buffer).await {
        if read == 0 {
            break;
        }
        if faults.blackhole.load(Ordering::SeqCst) {
            continue;
        }
        let Some(bytes) = buffer.get(..read) else {
            break;
        };
        if to.write_all(bytes).await.is_err() {
            break;
        }
    }
    // Pass the end of stream on, as a real link would.
    let _ = to.shutdown().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    async fn echo_server() -> io::Result<SocketAddr> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(echo(stream));
            }
        });
        Ok(address)
    }

    async fn echo(stream: TcpStream) {
        let (mut read, mut write) = stream.into_split();
        let _ = tokio::io::copy(&mut read, &mut write).await;
    }

    async fn round_trip(stream: &mut TcpStream) -> io::Result<[u8; 4]> {
        stream.write_all(b"ping").await?;
        let mut reply = [0; 4];
        stream.read_exact(&mut reply).await?;
        Ok(reply)
    }

    #[tokio::test]
    async fn relays_cuts_blackholes_and_refuses() -> io::Result<()> {
        let link = FaultyLink::start(echo_server().await?).await?;
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, link.port)).await?;
        assert_eq!(&round_trip(&mut stream).await?, b"ping");

        link.set_blackhole(true);
        let silent = tokio::time::timeout(Duration::from_millis(200), round_trip(&mut stream));
        assert!(silent.await.is_err(), "a black-holed link must not answer");
        link.set_blackhole(false);

        link.cut();
        let mut end = [0; 1];
        assert_eq!(stream.read(&mut end).await.unwrap_or(0), 0);

        link.set_refusing(true);
        let mut refused = TcpStream::connect((Ipv4Addr::LOCALHOST, link.port)).await?;
        assert_eq!(refused.read(&mut end).await.unwrap_or(0), 0);
        link.set_refusing(false);
        let mut fresh = TcpStream::connect((Ipv4Addr::LOCALHOST, link.port)).await?;
        assert_eq!(&round_trip(&mut fresh).await?, b"ping");
        Ok(())
    }
}
