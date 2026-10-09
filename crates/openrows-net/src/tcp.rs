// SPDX-License-Identifier: Apache-2.0
//! Socket options for every TCP connection a route opens (docs/24 §3,
//! docs/14 §4): no Nagle delay, and OS keepalive probes so a peer that
//! vanished (sleep, network change, a dead NAT entry) is noticed even while
//! a long statement keeps the connection busy.

use std::{io, time::Duration};

use socket2::{SockRef, TcpKeepalive};
use tokio::net::TcpStream;

/// Idle time before the OS sends the first keepalive probe (docs/14 §4).
const KEEPALIVE_IDLE: Duration = Duration::from_secs(30);
/// Time between unanswered probes, where the platform lets us choose it.
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(10);

/// Disable Nagle's algorithm and turn on TCP keepalive.
///
/// # Errors
/// The OS rejected a socket option.
pub fn configure_tcp(stream: &TcpStream) -> io::Result<()> {
    stream.set_nodelay(true)?;
    let keepalive = TcpKeepalive::new().with_time(KEEPALIVE_IDLE);
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    let keepalive = keepalive.with_interval(KEEPALIVE_INTERVAL);
    SockRef::from(stream).set_tcp_keepalive(&keepalive)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn connected_sockets_get_keepalive_and_no_delay() -> io::Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let stream = TcpStream::connect(listener.local_addr()?).await?;
        configure_tcp(&stream)?;
        let socket = SockRef::from(&stream);
        assert!(socket.keepalive()?);
        assert!(stream.nodelay()?);
        Ok(())
    }
}
