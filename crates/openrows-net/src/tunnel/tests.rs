// SPDX-License-Identifier: Apache-2.0
use std::{sync::Arc, time::Duration};

use russh::keys::{Algorithm, EcdsaCurve};
use tokio::net::TcpListener;

use super::{SshHop, SshTunnel, TunnelOptions, connect::host_key_order};
use crate::{
    AuthMethod, BoxFuture, Challenge, FirstUseDecision, HostKeyPolicy, KeyboardInteractivePrompter,
    PromptCancelled, SecretString, SshError, Stage, Timeouts, UnknownHost, known_hosts::KnownHosts,
};

struct RejectAll;

impl HostKeyPolicy for RejectAll {
    fn on_unknown_host<'a>(&'a self, _host: &'a UnknownHost) -> BoxFuture<'a, FirstUseDecision> {
        Box::pin(async { FirstUseDecision::Reject })
    }
}

impl KeyboardInteractivePrompter for RejectAll {
    fn respond<'a>(
        &'a self,
        _challenge: &'a Challenge,
    ) -> BoxFuture<'a, Result<Vec<SecretString>, PromptCancelled>> {
        Box::pin(async { Err(PromptCancelled) })
    }
}

fn options(dir: &tempfile::TempDir) -> TunnelOptions {
    TunnelOptions {
        known_hosts: Arc::new(KnownHosts::new(Vec::new(), dir.path().join("known_hosts"))),
        host_key_policy: Arc::new(RejectAll),
        prompter: Arc::new(RejectAll),
        timeouts: Timeouts {
            handshake: Duration::from_millis(300),
            ..Timeouts::default()
        },
        proxy: None,
    }
}

fn hop(port: u16) -> SshHop {
    SshHop {
        host: "127.0.0.1".into(),
        port,
        user: "nobody".into(),
        auth: vec![AuthMethod::Password("synthetic".into())],
    }
}

#[test]
fn host_key_order_prefers_known_and_skips_rsa() {
    let p256 = Algorithm::Ecdsa {
        curve: EcdsaCurve::NistP256,
    };
    let order = host_key_order(std::slice::from_ref(&p256));
    assert_eq!(order.first(), Some(&p256));
    assert!(order.contains(&Algorithm::Ed25519));
    assert!(
        !order
            .iter()
            .any(|algorithm| matches!(algorithm, Algorithm::Rsa { .. }))
    );
}

#[test]
fn hop_from_settings_follows_openssh_method_order() {
    let settings = crate::config::HostSettings {
        alias: "db".into(),
        host_name: "10.0.0.5".into(),
        user: None,
        port: 2222,
        identity_files: vec!["/keys/id_ed25519".into()],
        identities_only: false,
        proxy_jump: Vec::new(),
    };
    let hop = SshHop::from_settings(&settings, "local-user");
    assert_eq!(
        (hop.host.as_str(), hop.port, hop.user.as_str()),
        ("10.0.0.5", 2222, "local-user")
    );
    let labels: Vec<&str> = hop.auth.iter().map(AuthMethod::label).collect();
    assert_eq!(labels, ["agent", "publickey", "keyboard-interactive"]);

    let strict = crate::config::HostSettings {
        identities_only: true,
        ..settings
    };
    let labels: Vec<&str> = SshHop::from_settings(&strict, "u")
        .auth
        .iter()
        .map(AuthMethod::label)
        .collect();
    assert_eq!(labels, ["publickey", "keyboard-interactive"]);
}

#[tokio::test]
async fn empty_route_is_rejected() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert!(matches!(
        SshTunnel::connect(&[], &options(&dir)).await,
        Err(SshError::EmptyRoute)
    ));
}

#[tokio::test]
async fn silent_server_hits_the_handshake_timeout() {
    let dir = tempfile::tempdir().expect("temp dir");
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("address").port();
    let _hold = tokio::spawn(async move { listener.accept().await });
    let result = SshTunnel::connect(&[hop(port)], &options(&dir)).await;
    assert!(
        matches!(
            result,
            Err(SshError::Timeout {
                stage: Stage::Handshake,
                ..
            })
        ),
        "{result:?}"
    );
}

#[tokio::test]
async fn refused_port_is_a_connect_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let port = {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        listener.local_addr().expect("address").port()
    };
    let result = SshTunnel::connect(&[hop(port)], &options(&dir)).await;
    assert!(
        matches!(result, Err(SshError::Connect { .. })),
        "{result:?}"
    );
}
