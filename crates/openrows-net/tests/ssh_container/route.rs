// SPDX-License-Identifier: Apache-2.0
//! A profile's SSH settings end to end: `plan_route` → `SshTunnel` →
//! `TunnelDialer`, the path every routed database connection takes (D-10).

use std::path::Path;
use std::sync::Arc;

use openrows_net::config::SshConfig;
use openrows_net::{
    Dialer, FirstUseDecision, PlanContext, SshAuth, SshRouteSpec, SshTunnel, TunnelDialer,
    plan_route,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::published;
use crate::support::{Container, Policy, Prompter, Workspace, docker_ready, password};

#[tokio::test]
async fn profile_route_dials_streams_through_one_shared_tunnel() {
    if !docker_ready() {
        return;
    }
    let work = Workspace::new();
    let secret = password();
    let server = Container::start(&published(&secret, ""));
    let spec = SshRouteSpec {
        host: "127.0.0.1".into(),
        port: Some(server.port()),
        user: Some("tester".into()),
        auth: SshAuth::Password(Some(secret.as_str().into())),
        jump_hosts: Vec::new(),
    };
    let config = SshConfig::default();
    let context = PlanContext {
        config: &config,
        home: Path::new("/nonexistent"),
        default_user: "nobody",
    };
    let hops = plan_route(&spec, context).expect("plan");
    let options = work.options(
        Policy::new(FirstUseDecision::AcceptAndRemember),
        Prompter::answering(None),
    );
    let tunnel = Arc::new(SshTunnel::connect(&hops, &options).await.expect("tunnel"));
    let dialer = TunnelDialer::new(Arc::clone(&tunnel), "127.0.0.1", 7000);
    assert_eq!(dialer.describe(), "SSH 127.0.0.1 → 127.0.0.1:7000");
    for payload in [b"first-stream".as_slice(), b"second-stream"] {
        let mut stream = dialer.dial().await.expect("dial");
        let mut greeting = vec![0; server.name.len() + 1];
        stream.read_exact(&mut greeting).await.expect("greeting");
        assert_eq!(
            String::from_utf8_lossy(&greeting),
            format!("{}\n", server.name)
        );
        stream.write_all(payload).await.expect("write");
        let mut echoed = vec![0; payload.len()];
        stream.read_exact(&mut echoed).await.expect("echo");
        assert_eq!(echoed, payload);
    }
    assert!(!tunnel.is_closed());
    drop(dialer);
    let Ok(tunnel) = Arc::try_unwrap(tunnel) else {
        unreachable!("the dialer released the tunnel")
    };
    tunnel.close().await.expect("close");
}
