// SPDX-License-Identifier: Apache-2.0
//! Authenticated loopback adapter: every upstream byte uses openrows-net.
use openrows_driver_api::{ConnectRequest, DriverError, SslMode, TlsStatus};
use openrows_net::{BoxStream, Dialer, TcpDialer, tls::TlsClient};
use std::{io, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::{JoinHandle, JoinSet},
};

pub(crate) struct Proxy {
    pub(crate) port: u16,
    pub(crate) password: String,
    task: JoinHandle<()>,
}
impl Drop for Proxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Proxy {
    pub(crate) async fn start(request: &ConnectRequest) -> Result<Self, DriverError> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        let password = uuid::Uuid::now_v7().to_string();
        let request = Arc::new(request.clone());
        let credential = password.clone();
        let task = tokio::spawn(async move {
            let mut streams = JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let Ok((stream, _)) = accepted else { break; };
                        if streams.len() >= 16 { continue; }
                        let request = request.clone();
                        let credential = credential.clone();
                        streams.spawn(async move { let _ = serve(stream, &request, &credential).await; });
                    }
                    _ = streams.join_next(), if !streams.is_empty() => {}
                }
            }
        });
        Ok(Self {
            port,
            password,
            task,
        })
    }
}

async fn serve(mut stream: TcpStream, request: &ConnectRequest, password: &str) -> io::Result<()> {
    let (host, port) =
        tokio::time::timeout(request.timeouts.auth, negotiate(&mut stream, password))
            .await
            .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))??;
    let (mut upstream, _) = upstream(request, &host, port)
        .await
        .map_err(io::Error::other)?;
    stream.write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0]).await?;
    tokio::io::copy_bidirectional(&mut stream, &mut upstream).await?;
    Ok(())
}

async fn negotiate(stream: &mut TcpStream, password: &str) -> io::Result<(String, u16)> {
    if stream.read_u8().await? != 5 {
        return Err(invalid());
    }
    let methods = bytes(stream).await?;
    if !methods.contains(&2) {
        return Err(invalid());
    }
    stream.write_all(&[5, 2]).await?;
    if stream.read_u8().await? != 1 {
        return Err(invalid());
    }
    let user = bytes(stream).await?;
    let supplied = bytes(stream).await?;
    if user != b"openrows" || supplied != password.as_bytes() {
        return Err(invalid());
    }
    stream.write_all(&[1, 0]).await?;
    let mut header = [0; 4];
    stream.read_exact(&mut header).await?;
    if header[..3] != [5, 1, 0] {
        return Err(invalid());
    }
    let host = match header[3] {
        3 => String::from_utf8(bytes(stream).await?).map_err(|_| invalid())?,
        1 => {
            let mut ip = [0; 4];
            stream.read_exact(&mut ip).await?;
            std::net::Ipv4Addr::from(ip).to_string()
        }
        4 => {
            let mut ip = [0; 16];
            stream.read_exact(&mut ip).await?;
            std::net::Ipv6Addr::from(ip).to_string()
        }
        _ => return Err(invalid()),
    };
    Ok((host, stream.read_u16().await?))
}
async fn bytes(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut value = vec![0; usize::from(stream.read_u8().await?)];
    stream.read_exact(&mut value).await?;
    Ok(value)
}
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "invalid local MongoDB proxy request",
    )
}

pub(crate) async fn upstream(
    request: &ConnectRequest,
    host: &str,
    port: u16,
) -> Result<(BoxStream, TlsStatus), DriverError> {
    let stream = dial(request, host, port).await?;
    secure(request, (host, port), stream).await
}

pub(crate) async fn dial(
    request: &ConnectRequest,
    host: &str,
    port: u16,
) -> Result<BoxStream, DriverError> {
    let dialer: Arc<dyn Dialer> = request.via.as_ref().map_or_else(
        || Arc::new(TcpDialer::new(host.into(), port)) as Arc<dyn Dialer>,
        |via| via.retarget(host, port).unwrap_or_else(|| via.clone()),
    );
    tokio::time::timeout(request.timeouts.tcp, dialer.dial())
        .await
        .map_err(|_| DriverError::Network(io::ErrorKind::TimedOut.into()))?
        .map_err(DriverError::Network)
}
pub(crate) async fn secure(
    request: &ConnectRequest,
    (host, port): (&str, u16),
    stream: BoxStream,
) -> Result<(BoxStream, TlsStatus), DriverError> {
    if matches!(request.ssl_mode, SslMode::Disable | SslMode::Allow) {
        return Ok((stream, TlsStatus::Disabled));
    }
    let tls = TlsClient::new(&request.tls.params(request.ssl_mode))
        .map_err(|e| DriverError::Tls(e.to_string()))?;
    let handshake = tokio::time::timeout(request.timeouts.tls, tls.handshake(stream, host)).await;
    match handshake {
        Ok(Ok((stream, details))) => Ok((Box::new(stream), TlsStatus::Encrypted(details))),
        _ if request.ssl_mode == SslMode::Prefer => Ok((
            dial(request, host, port).await?,
            TlsStatus::PlaintextFallback,
        )),
        Ok(Err(error)) => Err(DriverError::Tls(error.to_string())),
        Err(_) => Err(DriverError::Tls("MongoDB TLS handshake timed out".into())),
    }
}
