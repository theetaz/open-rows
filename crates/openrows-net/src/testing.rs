// SPDX-License-Identifier: Apache-2.0
//! An in-process SOCKS5 / HTTP `CONNECT` proxy for integration tests of
//! proxied routes (feature `test-proxy`, also used by this crate's tests).
//!
//! It records each requested destination exactly as the client sent it, so
//! tests can prove that host names reach the proxy unresolved (remote DNS).
//! [`FaultyLink`] is a toxiproxy-style TCP relay that cuts, black-holes or
//! refuses connections on demand (network loss and server restarts).

mod link;

pub use link::FaultyLink;

use std::{
    io,
    net::{Ipv4Addr, Ipv6Addr},
    sync::{Arc, Mutex, PoisonError},
};

use data_encoding::BASE64;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, copy_bidirectional},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

use crate::proxy::{ProxyAuth, ProxyKind, ProxySpec};

/// A running test proxy on `127.0.0.1`; stops when dropped.
pub struct TestProxy {
    /// Listening port.
    pub port: u16,
    config: TestProxyConfig,
    seen: Arc<Mutex<Vec<String>>>,
    task: JoinHandle<()>,
}

/// What a test proxy accepts.
#[derive(Clone, Debug)]
pub struct TestProxyConfig {
    /// Protocol spoken.
    pub kind: ProxyKind,
    /// Required `(user, password)`; `None` accepts anonymous clients.
    pub credentials: Option<(String, String)>,
}

impl TestProxy {
    /// Listen on an ephemeral loopback port.
    ///
    /// # Errors
    /// The listener cannot be bound.
    pub async fn start(config: TestProxyConfig) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        let served = config.clone();
        let task = tokio::spawn(async move {
            while let Ok((client, _)) = listener.accept().await {
                tokio::spawn(serve_logged(client, served.clone(), Arc::clone(&log)));
            }
        });
        Ok(Self {
            port,
            config,
            seen,
            task,
        })
    }

    /// A client spec for this proxy with its configured credentials.
    #[must_use]
    pub fn spec(&self) -> ProxySpec {
        ProxySpec {
            kind: self.config.kind,
            host: Ipv4Addr::LOCALHOST.to_string(),
            port: self.port,
            auth: self
                .config
                .credentials
                .as_ref()
                .map(|(user, password)| ProxyAuth {
                    user: user.clone(),
                    password: password.as_str().into(),
                }),
        }
    }

    /// Destinations requested so far, as `host:port` exactly as sent.
    #[must_use]
    pub fn destinations(&self) -> Vec<String> {
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Drop for TestProxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Serve one client; a failed test connection only affects the test that made it.
async fn serve_logged(client: TcpStream, config: TestProxyConfig, seen: Arc<Mutex<Vec<String>>>) {
    let _ = serve(client, &config, &seen).await;
}

async fn serve(
    mut client: TcpStream,
    config: &TestProxyConfig,
    seen: &Mutex<Vec<String>>,
) -> io::Result<()> {
    let destination = match config.kind {
        ProxyKind::Socks5 => socks5_request(&mut client, config).await?,
        ProxyKind::HttpConnect => http_request(&mut client, config).await?,
    };
    let Some((host, port)) = destination else {
        return Ok(());
    };
    seen.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(format!("{host}:{port}"));
    let upstream = TcpStream::connect((host.trim_matches(['[', ']']), port)).await;
    let mut upstream = match (config.kind, upstream) {
        (_, Ok(stream)) => stream,
        (ProxyKind::Socks5, Err(_)) => {
            return client.write_all(&[5, 5, 0, 1, 0, 0, 0, 0, 0, 0]).await;
        }
        (ProxyKind::HttpConnect, Err(_)) => {
            return client.write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n").await;
        }
    };
    let ok: &[u8] = match config.kind {
        ProxyKind::Socks5 => &[5, 0, 0, 1, 127, 0, 0, 1, 0, 0],
        ProxyKind::HttpConnect => b"HTTP/1.1 200 Connection established\r\n\r\n",
    };
    client.write_all(ok).await?;
    copy_bidirectional(&mut client, &mut upstream)
        .await
        .map(drop)
}

/// Greeting, optional RFC 1929 login and the `CONNECT` request.
async fn socks5_request(
    client: &mut TcpStream,
    config: &TestProxyConfig,
) -> io::Result<Option<(String, u16)>> {
    let [_, count] = read_array(client).await?;
    let mut methods = vec![0; usize::from(count)];
    client.read_exact(&mut methods).await?;
    if let Some((user, password)) = &config.credentials {
        if !methods.contains(&2) {
            client.write_all(&[5, 0xFF]).await?;
            return Ok(None);
        }
        client.write_all(&[5, 2]).await?;
        let [_, length] = read_array(client).await?;
        let sent_user = read_text(client, length).await?;
        let [length] = read_array(client).await?;
        let sent_password = read_text(client, length).await?;
        let accepted = &sent_user == user && &sent_password == password;
        client.write_all(&[1, u8::from(!accepted)]).await?;
        if !accepted {
            return Ok(None);
        }
    } else {
        client.write_all(&[5, 0]).await?;
    }
    let [_, _, _, address_type] = read_array(client).await?;
    let host = match address_type {
        1 => Ipv4Addr::from(read_array::<4>(client).await?).to_string(),
        4 => format!("[{}]", Ipv6Addr::from(read_array::<16>(client).await?)),
        _ => {
            let [length] = read_array(client).await?;
            read_text(client, length).await?
        }
    };
    let port = u16::from_be_bytes(read_array(client).await?);
    Ok(Some((host, port)))
}

/// `CONNECT host:port`, checking Basic credentials when required.
async fn http_request(
    client: &mut TcpStream,
    config: &TestProxyConfig,
) -> io::Result<Option<(String, u16)>> {
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") && head.len() < 8192 {
        head.push(client.read_u8().await?);
    }
    let head = String::from_utf8_lossy(&head).into_owned();
    if let Some((user, password)) = &config.credentials {
        let expected = format!(
            "Proxy-Authorization: Basic {}",
            BASE64.encode(format!("{user}:{password}").as_bytes())
        );
        if !head.lines().any(|line| line == expected) {
            client
                .write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\n\r\n")
                .await?;
            return Ok(None);
        }
    }
    let target = head.split(' ').nth(1).unwrap_or_default();
    let parsed = target
        .rsplit_once(':')
        .and_then(|(host, port)| Some((host.to_owned(), port.parse().ok()?)));
    if parsed.is_none() {
        client
            .write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n")
            .await?;
    }
    Ok(parsed)
}

async fn read_array<const N: usize>(stream: &mut TcpStream) -> io::Result<[u8; N]> {
    let mut buffer = [0; N];
    stream.read_exact(&mut buffer).await?;
    Ok(buffer)
}

async fn read_text(stream: &mut TcpStream, length: u8) -> io::Result<String> {
    let mut buffer = vec![0; usize::from(length)];
    stream.read_exact(&mut buffer).await?;
    Ok(String::from_utf8_lossy(&buffer).into_owned())
}
