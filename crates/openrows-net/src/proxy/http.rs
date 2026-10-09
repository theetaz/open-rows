// SPDX-License-Identifier: Apache-2.0
//! HTTP `CONNECT` tunnel (RFC 9110 §9.3.6) with optional Basic proxy
//! authentication. The response head is read byte by byte so no byte of the
//! tunnelled protocol is consumed.

use data_encoding::BASE64;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use zeroize::Zeroizing;

use super::{ProxyError, ProxySpec};

/// Longest response head accepted from the proxy.
const HEAD_LIMIT: usize = 8 * 1024;

/// Send `CONNECT host:port` on `stream` and check the proxy's answer.
pub(super) async fn negotiate<S>(
    stream: &mut S,
    proxy: &ProxySpec,
    host: &str,
    port: u16,
) -> Result<(), ProxyError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let name = proxy.describe();
    let io = |source| ProxyError::Io {
        proxy: name.clone(),
        source,
    };
    let destination = authority(host, port);
    stream
        .write_all(request(proxy, &destination).as_bytes())
        .await
        .map_err(io)?;
    let head = read_head(stream).await.map_err(|error| match error {
        HeadError::Io(source) => io(source),
        HeadError::TooLong => ProxyError::Protocol {
            proxy: name.clone(),
            reason: "response head is longer than 8 KiB".to_owned(),
        },
    })?;
    check_status(&head, proxy, &destination)
}

/// `host:port`, with IPv6 literals in brackets.
pub(super) fn authority(host: &str, port: u16) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// The request head; zeroized because it may carry the password.
pub(super) fn request(proxy: &ProxySpec, destination: &str) -> Zeroizing<String> {
    let mut head = Zeroizing::new(format!(
        "CONNECT {destination} HTTP/1.1\r\nHost: {destination}\r\n"
    ));
    if let Some(auth) = &proxy.auth {
        let credentials = Zeroizing::new(format!("{}:{}", auth.user, auth.password.expose()));
        let encoded = Zeroizing::new(BASE64.encode(credentials.as_bytes()));
        head.push_str("Proxy-Authorization: Basic ");
        head.push_str(&encoded);
        head.push_str("\r\n");
    }
    head.push_str("\r\n");
    head
}

enum HeadError {
    Io(std::io::Error),
    TooLong,
}

async fn read_head<S: AsyncRead + Unpin>(stream: &mut S) -> Result<String, HeadError> {
    let mut head = Vec::with_capacity(256);
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() >= HEAD_LIMIT {
            return Err(HeadError::TooLong);
        }
        head.push(stream.read_u8().await.map_err(HeadError::Io)?);
    }
    Ok(String::from_utf8_lossy(&head).into_owned())
}

/// `HTTP/1.x 2xx` opens the tunnel; 407 is an authentication problem.
pub(super) fn check_status(
    head: &str,
    proxy: &ProxySpec,
    destination: &str,
) -> Result<(), ProxyError> {
    let line = head.lines().next().unwrap_or_default();
    let mut parts = line.splitn(3, ' ');
    let version = parts.next().unwrap_or_default();
    let status = parts.next().and_then(|code| code.parse::<u16>().ok());
    let name = proxy.describe();
    let Some(status) = status.filter(|_| version.starts_with("HTTP/1.")) else {
        return Err(ProxyError::Protocol {
            proxy: name,
            reason: format!("unexpected status line {:?}", truncate(line)),
        });
    };
    match status {
        200..=299 => Ok(()),
        407 if proxy.auth.is_none() => Err(ProxyError::AuthRequired { proxy: name }),
        407 => Err(ProxyError::AuthFailed { proxy: name }),
        _ => Err(ProxyError::Refused {
            proxy: name,
            destination: destination.to_owned(),
            reason: format!(
                "HTTP {}",
                truncate(line.split_once(' ').map_or("", |s| s.1))
            ),
        }),
    }
}

fn truncate(text: &str) -> &str {
    text.char_indices()
        .nth(80)
        .map_or(text, |(at, _)| &text[..at])
}
