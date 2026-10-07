// SPDX-License-Identifier: Apache-2.0
//! Host-key hard stops and a two-hop `ProxyJump` route from an SSH config alias.

use std::path::Path;

use openrows_net::{AuthMethod, FirstUseDecision, SshError, SshHop, SshTunnel, resolve_route};

use crate::support::{
    Container, ContainerSpec, Network, Policy, Prompter, Workspace, docker_ready, login_and_echo,
    password,
};
use crate::{hop, published};

fn password_hop(server: &Container, secret: &str) -> SshHop {
    hop(server, "tester", vec![AuthMethod::Password(secret.into())])
}

#[tokio::test]
async fn rejected_first_use_aborts_without_recording() {
    if !docker_ready() {
        return;
    }
    let work = Workspace::new();
    let secret = password();
    let server = Container::start(&published(&secret, ""));
    let reject = Policy::new(FirstUseDecision::Reject);
    let options = work.options(reject.clone(), Prompter::answering(None));
    let result = SshTunnel::connect(&[password_hop(&server, &secret)], &options).await;
    assert!(
        matches!(result, Err(SshError::HostKeyRejected { .. })),
        "{result:?}"
    );
    assert_eq!(reject.seen().len(), 1);
    assert!(
        !work.known_hosts().exists(),
        "a rejected key is not recorded"
    );
}

#[tokio::test]
async fn changed_host_key_is_a_hard_stop() {
    if !docker_ready() {
        return;
    }
    let work = Workspace::new();
    let secret = password();
    let server = Container::start(&published(&secret, ""));
    let impostor = work.keygen("impostor", &["-t", "ed25519"], "");
    let recorded = format!("[127.0.0.1]:{} {}\n", server.port(), impostor.public);
    std::fs::write(work.known_hosts(), &recorded).expect("write known_hosts");

    let accept = Policy::new(FirstUseDecision::AcceptAndRemember);
    let options = work.options(accept.clone(), Prompter::answering(None));
    let result = SshTunnel::connect(&[password_hop(&server, &secret)], &options).await;
    assert!(
        matches!(&result, Err(SshError::HostKeyChanged { at, .. }) if at.line == 1),
        "{result:?}"
    );
    assert!(
        accept.seen().is_empty(),
        "a changed key never reaches the first-use prompt"
    );
    let unchanged = std::fs::read_to_string(work.known_hosts()).expect("read");
    assert_eq!(unchanged, recorded);
}

/// `jump` is the published bastion; `db` is only reachable on the Docker network.
fn jump_config(bastion_port: u16, identity: &Path) -> String {
    let identity = identity.display();
    format!(
        "Host jump
  HostName 127.0.0.1
  Port {bastion_port}
  User tester
  IdentityFile {identity}
  IdentitiesOnly yes
Host db
  HostName db-internal
  User tester
  IdentityFile {identity}
  IdentitiesOnly yes
  ProxyJump jump
"
    )
}

#[tokio::test]
async fn two_hop_proxy_jump_from_ssh_config_alias() {
    if !docker_ready() {
        return;
    }
    let work = Workspace::new();
    let key = work.keygen("jump-key", &["-t", "ed25519"], "");
    let secret = password();
    let network = Network::create();
    let bastion = Container::start(&ContainerSpec {
        network: Some((&network, "bastion")),
        ..published(&secret, &key.public)
    });
    let database_host = Container::start(&ContainerSpec {
        publish: false,
        network: Some((&network, "db-internal")),
        ..published(&secret, &key.public)
    });
    let config = work.ssh_config(&jump_config(bastion.port(), &key.private));
    let route = resolve_route(&config, "db").expect("route");
    let hops: Vec<SshHop> = route
        .iter()
        .map(|settings| SshHop::from_settings(settings, "unused"))
        .collect();
    let hosts: Vec<&str> = hops.iter().map(|hop| hop.host.as_str()).collect();
    assert_eq!(hosts, ["127.0.0.1", "db-internal"]);

    let policy = Policy::new(FirstUseDecision::AcceptAndRemember);
    let options = work.options(policy.clone(), Prompter::answering(None));
    login_and_echo(&hops, &options, &database_host.name, "two-hop").await;
    let prompted: Vec<String> = policy.seen().into_iter().map(|host| host.host).collect();
    assert_eq!(prompted, ["127.0.0.1", "db-internal"]);
}
