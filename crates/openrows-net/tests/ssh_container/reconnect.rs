// SPDX-License-Identifier: Apache-2.0
//! A dropped tunnel is re-established on the next dial (docs/24 §3), with a
//! clear error when the server is gone; host keys accepted once are reported.
#![allow(
    clippy::expect_used,
    reason = "test helpers fail fast with a message, like the tests that call them"
)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use openrows_net::{Destination, Dialer, FirstUseDecision, SharedTunnel, SshTunnel, TunnelDialer};

use crate::published;
use crate::route::{greet_and_echo, tester_hops, trusting};
use crate::support::{Container, Policy, Prompter, Workspace, docker_ready, password};

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
