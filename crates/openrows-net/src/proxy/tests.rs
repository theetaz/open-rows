// SPDX-License-Identifier: Apache-2.0
use std::{net::Ipv4Addr, sync::Arc};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

use super::{ProxyAuth, ProxyDialer, ProxyError, ProxyKind, ProxySpec, http, socks5};
use crate::{
    Dialer, Timeouts,
    testing::{TestProxy, TestProxyConfig},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// An echo server on loopback; returns its port.
async fn echo_server() -> std::io::Result<u16> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let (mut read, mut write) = stream.split();
                let _ = tokio::io::copy(&mut read, &mut write).await;
            });
        }
    });
    Ok(port)
}

async fn proxy(kind: ProxyKind, credentials: Option<(&str, &str)>) -> std::io::Result<TestProxy> {
    TestProxy::start(TestProxyConfig {
        kind,
        credentials: credentials.map(|(user, password)| (user.to_owned(), password.to_owned())),
    })
    .await
}

fn with_auth(spec: &ProxySpec, auth: Option<(&str, &str)>) -> ProxySpec {
    ProxySpec {
        auth: auth.map(|(user, password)| ProxyAuth {
            user: user.to_owned(),
            password: password.into(),
        }),
        ..spec.clone()
    }
}

async fn round_trip(spec: &ProxySpec, host: &str, port: u16) -> Result<(), ProxyError> {
    let mut stream = spec.connect(host, port, &Timeouts::default()).await?;
    stream.write_all(b"ping-through-proxy").await.ok();
    let mut echoed = [0; 18];
    stream.read_exact(&mut echoed).await.ok();
    assert_eq!(&echoed, b"ping-through-proxy");
    Ok(())
}

#[tokio::test]
async fn both_proxies_tunnel_streams_and_leave_names_to_the_proxy() -> TestResult {
    let target = echo_server().await?;
    for kind in [ProxyKind::Socks5, ProxyKind::HttpConnect] {
        let server = proxy(kind, None).await?;
        round_trip(&server.spec(), "localhost", target).await?;
        round_trip(&server.spec(), "127.0.0.1", target).await?;
        assert_eq!(
            server.destinations(),
            [format!("localhost:{target}"), format!("127.0.0.1:{target}")],
            "{kind}: names reach the proxy unresolved"
        );
    }
    Ok(())
}

#[tokio::test]
async fn credentials_are_sent_and_checked() -> TestResult {
    let target = echo_server().await?;
    for kind in [ProxyKind::Socks5, ProxyKind::HttpConnect] {
        let server = proxy(kind, Some(("ops", "proxy-pw-synthetic"))).await?;
        round_trip(&server.spec(), "localhost", target).await?;
        let wrong = with_auth(&server.spec(), Some(("ops", "wrong")));
        let failure = round_trip(&wrong, "localhost", target).await;
        assert!(
            matches!(failure, Err(ProxyError::AuthFailed { .. })),
            "{kind}: {failure:?}"
        );
        let anonymous = with_auth(&server.spec(), None);
        let failure = round_trip(&anonymous, "localhost", target).await;
        assert!(
            matches!(failure, Err(ProxyError::AuthRequired { .. })),
            "{kind}: {failure:?}"
        );
        assert!(failure.is_err_and(|error| error.is_auth()));
    }
    Ok(())
}

#[tokio::test]
async fn refusals_and_unreachable_proxies_are_typed() -> TestResult {
    let closed = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let closed_port = closed.local_addr()?.port();
    drop(closed);
    for kind in [ProxyKind::Socks5, ProxyKind::HttpConnect] {
        let server = proxy(kind, None).await?;
        let refused = round_trip(&server.spec(), "127.0.0.1", closed_port).await;
        assert!(
            matches!(&refused, Err(ProxyError::Refused { destination, .. })
                if destination == &format!("127.0.0.1:{closed_port}")),
            "{kind}: {refused:?}"
        );
    }
    let nowhere = ProxySpec {
        kind: ProxyKind::Socks5,
        host: "127.0.0.1".to_owned(),
        port: closed_port,
        auth: None,
    };
    let unreachable = round_trip(&nowhere, "db", 5432).await;
    assert!(matches!(unreachable, Err(ProxyError::Unreachable { .. })));
    assert_eq!(
        unreachable.map_err(|error| error.to_string()),
        Err(format!(
            "could not reach SOCKS5 proxy 127.0.0.1:{closed_port}"
        ))
    );
    Ok(())
}

#[tokio::test]
async fn the_dialer_names_the_route_and_opens_streams() -> TestResult {
    let target = echo_server().await?;
    let server = proxy(ProxyKind::HttpConnect, None).await?;
    let dialer = ProxyDialer::new(Arc::new(server.spec()), "localhost", target);
    assert_eq!(
        dialer.describe(),
        format!("HTTP proxy 127.0.0.1:{} → localhost:{target}", server.port)
    );
    let mut stream = dialer.dial().await?;
    stream.write_all(b"x").await?;
    assert_eq!(stream.read_u8().await?, b'x');
    Ok(())
}

#[test]
fn socks5_requests_encode_ips_and_names() -> Result<(), ProxyError> {
    assert_eq!(
        socks5::connect_request("10.0.0.5", 5432)?,
        [5, 1, 0, 1, 10, 0, 0, 5, 0x15, 0x38]
    );
    let v6 = socks5::connect_request("[::1]", 3306)?;
    assert_eq!((v6[3], v6.len()), (4, 4 + 16 + 2));
    let name = socks5::connect_request("db.internal", 5432)?;
    assert_eq!(&name[3..5], &[3, 11]);
    assert_eq!(&name[5..16], b"db.internal");
    assert!(socks5::connect_request(&"a".repeat(256), 1).is_err());
    assert_eq!(socks5::reply_text(5), "connection refused");
    Ok(())
}

#[test]
fn http_heads_carry_basic_auth_and_statuses_map_to_errors() {
    let spec = ProxySpec {
        kind: ProxyKind::HttpConnect,
        host: "proxy".to_owned(),
        port: 3128,
        auth: Some(ProxyAuth {
            user: "ops".to_owned(),
            password: "pw-synthetic".into(),
        }),
    };
    assert_eq!(http::authority("::1", 5432), "[::1]:5432");
    assert_eq!(http::authority("db", 5432), "db:5432");
    let head = http::request(&spec, "db:5432");
    assert!(head.starts_with("CONNECT db:5432 HTTP/1.1\r\nHost: db:5432\r\n"));
    assert!(head.contains("Proxy-Authorization: Basic b3BzOnB3LXN5bnRoZXRpYw==\r\n"));
    assert!(!format!("{spec:?}").contains("pw-synthetic"));
    assert!(http::check_status("HTTP/1.0 200 OK\r\n\r\n", &spec, "db:5432").is_ok());
    let denied = http::check_status("HTTP/1.1 403 Forbidden\r\n\r\n", &spec, "db:5432");
    assert_eq!(
        denied.map_err(|error| error.to_string()),
        Err("HTTP proxy proxy:3128 could not open db:5432: HTTP 403 Forbidden".to_owned())
    );
    let garbage = http::check_status("SSH-2.0-OpenSSH\r\n\r\n", &spec, "db:5432");
    assert!(matches!(garbage, Err(ProxyError::Protocol { .. })));
}
