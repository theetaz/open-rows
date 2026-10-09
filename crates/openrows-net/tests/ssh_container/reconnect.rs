// SPDX-License-Identifier: Apache-2.0
//! A dropped tunnel is re-established on the next dial (docs/24 §3), with a
//! clear error when the server is gone; an on-demand probe (after wake or a
//! network change, M1-08) drops a tunnel that is cut or no longer answers;
//! host keys accepted once are reported.
#![allow(
    clippy::expect_used,
    reason = "test helpers fail fast with a message, like the tests that call them"
)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use openrows_net::{Destination, Dialer, FirstUseDecision, SharedTunnel, SshTunnel, TunnelDialer};
use tokio::io::AsyncReadExt;

use crate::published;
use crate::route::{greet_and_echo, tester_hops, trusting};
use crate::support::{Container, Policy, Prompter, Workspace, docker_ready, password};

/// How long an on-demand tunnel probe may wait for the server's reply.
const PROBE: Duration = Duration::from_secs(1);

/// Wait (bounded) until the shared tunnel notices its transport closed.
async fn wait_closed(shared: &SharedTunnel) {
    let tunnel = shared.current().expect("connected");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !tunnel.is_closed() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(tunnel.is_closed(), "the cut session was noticed");
}

#[tokio::test]
async fn a_dropped_tunnel_reconnects_on_the_next_dial() {
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
    let dialer = TunnelDialer::new(
        Arc::clone(&shared),
        Destination::Tcp {
            host: "127.0.0.1".into(),
            port: 7000,
        },
    );
    let mut stream = dialer.dial().await.expect("first dial");
    greet_and_echo(&mut stream, &server.name, b"before-the-drop").await;
    drop(stream);
    let first = shared.current().expect("connected");
    assert!(
        server.exec(&["pkill", "-f", "sshd.*tester"]),
        "cut the session"
    );
    wait_closed(&shared).await;
    let mut stream = dialer.dial().await.expect("dial after the drop");
    greet_and_echo(&mut stream, &server.name, b"after-the-drop").await;
    let second = shared.current().expect("reconnected");
    assert!(
        !Arc::ptr_eq(&first, &second),
        "a new tunnel replaced the dead one"
    );
    drop((stream, first, second));
    server.kill();
    wait_closed(&shared).await;
    let error = dialer.dial().await.err().expect("server gone");
    assert!(
        error
            .to_string()
            .contains("the SSH tunnel is closed and could not be re-established"),
        "{error}"
    );
}

#[tokio::test]
async fn keys_accepted_once_are_reported_and_not_remembered() {
    if !docker_ready() {
        return;
    }
    let work = Workspace::new();
    let secret = password();
    let server = Container::start(&published(&secret, ""));
    let options = work.options(
        Policy::new(FirstUseDecision::AcceptOnce),
        Prompter::answering(None),
    );
    let tunnel = SshTunnel::connect(&tester_hops(&server, &secret), &options)
        .await
        .expect("tunnel");
    assert_eq!(tunnel.unremembered_host_keys(), ["127.0.0.1"]);
    assert!(!work.known_hosts().exists(), "nothing was written");
    tunnel.close().await.expect("close");
    let remembered = SshTunnel::connect(&tester_hops(&server, &secret), &trusting(&work))
        .await
        .expect("tunnel");
    assert!(remembered.unremembered_host_keys().is_empty());
    remembered.close().await.expect("close");
}

/// A dialer to the container's echo service through `shared`.
fn echo_dialer(shared: &Arc<SharedTunnel>) -> TunnelDialer {
    TunnelDialer::new(
        Arc::clone(shared),
        Destination::Tcp {
            host: "127.0.0.1".into(),
            port: 7000,
        },
    )
}

#[tokio::test]
async fn a_probe_drops_a_tunnel_whose_server_stopped_answering() {
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
    let dialer = echo_dialer(&shared);
    let mut stream = dialer.dial().await.expect("first dial");
    greet_and_echo(&mut stream, &server.name, b"before-the-pause").await;
    drop(stream);
    let first = shared.current().expect("connected");
    assert!(shared.probe(PROBE).await, "a live tunnel answers");
    assert!(Arc::ptr_eq(&first, &shared.current().expect("kept")));
    server.pause();
    let probing = Instant::now();
    assert!(
        !shared.probe(PROBE).await,
        "a frozen server does not answer"
    );
    assert!(probing.elapsed() < PROBE + Duration::from_millis(500));
    assert!(shared.current().is_none(), "the dead tunnel was dropped");
    server.unpause();
    let mut stream = dialer.dial().await.expect("dial after the outage");
    greet_and_echo(&mut stream, &server.name, b"after-the-pause").await;
    let second = shared.current().expect("reconnected");
    assert!(
        !Arc::ptr_eq(&first, &second),
        "a new tunnel serves the dial"
    );
}

#[tokio::test]
async fn a_probe_notices_a_cut_tunnel_at_once() {
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
    let dialer = echo_dialer(&shared);
    let mut stream = dialer.dial().await.expect("first dial");
    greet_and_echo(&mut stream, &server.name, b"before-the-cut").await;
    assert!(
        server.exec(&["pkill", "-f", "sshd.*tester"]),
        "cut the session"
    );
    let mut end = [0; 1];
    let read = stream.read(&mut end).await.unwrap_or(0);
    assert_eq!(read, 0, "the forwarded stream ends with the tunnel");
    assert!(!shared.probe(PROBE).await, "a cut tunnel fails its probe");
    assert!(shared.current().is_none());
    let mut stream = dialer.dial().await.expect("dial after the cut");
    greet_and_echo(&mut stream, &server.name, b"after-the-cut").await;
}
