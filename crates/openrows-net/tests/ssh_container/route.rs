// SPDX-License-Identifier: Apache-2.0
//! A profile's SSH settings end to end: `plan_route` → `SharedTunnel` →
//! `TunnelDialer`, the path every routed database connection takes (D-10),
//! to TCP and Unix-socket destinations and behind a proxy (M1-06).
#![allow(
    clippy::expect_used,
    reason = "shared test helpers fail fast with a message, like the tests that call them"
)]

use std::path::Path;
use std::sync::Arc;

use openrows_net::config::SshConfig;
use openrows_net::testing::{TestProxy, TestProxyConfig};
use openrows_net::{
    BoxStream, Destination, Dialer, FirstUseDecision, PlanContext, ProxyKind, SharedTunnel,
    SshAuth, SshHop, SshRouteSpec, SshTunnel, TunnelDialer, TunnelOptions, plan_route,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::published;
use crate::support::{Container, Policy, Prompter, Workspace, docker_ready, note, password};

/// Hops for `tester` with `secret` on a published container.
pub fn tester_hops(server: &Container, secret: &str) -> Vec<SshHop> {
    let spec = SshRouteSpec {
        host: "127.0.0.1".into(),
        port: Some(server.port()),
        user: Some("tester".into()),
        auth: SshAuth::Password(Some(secret.into())),
        jump_hosts: Vec::new(),
    };
    let config = SshConfig::default();
    let context = PlanContext {
        config: &config,
        home: Path::new("/nonexistent"),
        default_user: "nobody",
    };
    plan_route(&spec, context).expect("plan")
}

/// Options that trust and remember first-use keys.
pub fn trusting(work: &Workspace) -> TunnelOptions {
    work.options(
        Policy::new(FirstUseDecision::AcceptAndRemember),
        Prompter::answering(None),
    )
}

/// Read the greeting (the container's hostname) and echo `payload`.
pub async fn greet_and_echo(stream: &mut BoxStream, host: &str, payload: &[u8]) {
    let mut greeting = vec![0; host.len() + 1];
    stream.read_exact(&mut greeting).await.expect("greeting");
    assert_eq!(String::from_utf8_lossy(&greeting), format!("{host}\n"));
    stream.write_all(payload).await.expect("write");
    let mut echoed = vec![0; payload.len()];
    stream.read_exact(&mut echoed).await.expect("echo");
    assert_eq!(echoed, payload);
}

#[tokio::test]
async fn profile_route_dials_streams_through_one_shared_tunnel() {
    if !docker_ready() {
        return;
    }
    let work = Workspace::new();
    let secret = password();
    let server = Container::start(&published(&secret, ""));
    let shared = Arc::new(SharedTunnel::new(
        tester_hops(&server, &secret),
        trusting(&work),
    ));
    let tcp = Destination::Tcp {
        host: "127.0.0.1".into(),
        port: 7000,
    };
    let dialer = TunnelDialer::new(Arc::clone(&shared), tcp);
    assert_eq!(dialer.describe(), "SSH 127.0.0.1 → 127.0.0.1:7000");
    assert!(!dialer.is_socket());
    for payload in [b"first-stream".as_slice(), b"second-stream"] {
        let mut stream = dialer.dial().await.expect("dial");
        greet_and_echo(&mut stream, &server.name, payload).await;
    }
    let first = shared.current().expect("connected");
    let mut again = dialer.dial().await.expect("dial");
    greet_and_echo(&mut again, &server.name, b"same-tunnel").await;
    let second = shared.current().expect("connected");
    assert!(Arc::ptr_eq(&first, &second), "one tunnel serves every dial");
    drop((first, second, again, dialer));
    shared.close().await;
    assert!(shared.current().is_none());
}

#[tokio::test]
async fn unix_sockets_on_the_server_are_reached_with_streamlocal() {
    if !docker_ready() {
        return;
    }
    let work = Workspace::new();
    let secret = password();
    let server = Container::start(&published(&secret, ""));
    let shared = Arc::new(SharedTunnel::new(
        tester_hops(&server, &secret),
        trusting(&work),
    ));
    let socket = Destination::Socket("/tmp/openrows-echo.sock".into());
    let dialer = TunnelDialer::new(Arc::clone(&shared), socket);
    assert!(dialer.is_socket());
    assert_eq!(dialer.describe(), "SSH 127.0.0.1 → /tmp/openrows-echo.sock");
    let mut stream = dialer.dial().await.expect("dial socket");
    greet_and_echo(&mut stream, &server.name, b"over-a-unix-socket").await;
    let missing = TunnelDialer::new(shared, Destination::Socket("/tmp/missing.sock".into()));
    let error = missing.dial().await.err().expect("missing socket fails");
    assert!(error.to_string().contains("/tmp/missing.sock"), "{error}");
}

#[tokio::test]
async fn proxies_carry_the_first_ssh_hop() {
    if !docker_ready() {
        return;
    }
    let secret = password();
    let server = Container::start(&published(&secret, ""));
    for kind in [ProxyKind::Socks5, ProxyKind::HttpConnect] {
        let work = Workspace::new();
        let proxy = TestProxy::start(TestProxyConfig {
            kind,
            credentials: Some(("ops".into(), "proxy-pw-synthetic".into())),
        })
        .await
        .expect("proxy");
        let options = TunnelOptions {
            proxy: Some(Arc::new(proxy.spec())),
            ..trusting(&work)
        };
        let tunnel = SshTunnel::connect(&tester_hops(&server, &secret), &options)
            .await
            .expect("tunnel through proxy");
        let mut stream: BoxStream =
            Box::new(tunnel.open_stream("127.0.0.1", 7000).await.expect("stream"));
        greet_and_echo(&mut stream, &server.name, b"via-proxy").await;
        assert_eq!(
            proxy.destinations(),
            [format!("127.0.0.1:{}", server.port())]
        );
        note(&format!(
            "timing {kind} proxy + SSH: connect={:.1}ms handshake+auth={:.1}ms",
            tunnel.timings()[0].connect.as_secs_f64() * 1e3,
            tunnel.timings()[0].handshake.as_secs_f64() * 1e3
        ));
        drop(stream);
        tunnel.close().await.expect("close");
    }
}
