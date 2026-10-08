// SPDX-License-Identifier: Apache-2.0
#![allow(
    clippy::expect_used,
    reason = "test fixtures fail fast with a message, like the tests that call them"
)]

use std::path::{Path, PathBuf};

use super::{PlanContext, SshAuth, SshRouteSpec, plan_route};
use crate::{
    AuthMethod, RouteError, SecretString, SshHop,
    config::{FileSystem, JumpSpec, SshConfig},
};

const CONFIG: &str = "Host bastion
  HostName bastion.example.com
  User ops
  Port 2222
  IdentityFile ~/.ssh/bastion_ed25519
Host db-box
  HostName 10.0.0.7
  User app
  IdentityFile ~/.ssh/app_ed25519
  IdentitiesOnly yes
  ProxyJump bastion
";

fn config(text: &str) -> SshConfig {
    SshConfig::parse(text, Path::new("/cfg"), Path::new("/home/me"), &FileSystem).expect("parses")
}

fn plan(spec: &SshRouteSpec, text: &str) -> Result<Vec<SshHop>, RouteError> {
    let config = config(text);
    plan_route(
        spec,
        PlanContext {
            config: &config,
            home: Path::new("/home/me"),
            default_user: "local",
        },
    )
}

fn spec(host: &str, auth: SshAuth) -> SshRouteSpec {
    SshRouteSpec {
        host: host.to_owned(),
        port: None,
        user: None,
        auth,
        jump_hosts: Vec::new(),
    }
}

/// `user@host:port` per hop.
fn hosts(hops: &[SshHop]) -> Vec<String> {
    hops.iter()
        .map(|hop| format!("{}@{}:{}", hop.user, hop.host, hop.port))
        .collect()
}

/// Method labels, with key paths, per hop.
fn methods(hop: &SshHop) -> Vec<String> {
    hop.auth
        .iter()
        .map(|method| match method {
            AuthMethod::PrivateKey { path, .. } => format!("key:{}", path.display()),
            other => other.label().to_owned(),
        })
        .collect()
}

#[test]
fn a_plain_host_is_one_hop_with_defaults() {
    let hops = plan(&spec("ssh.example.com", SshAuth::Password(None)), "").expect("route");
    assert_eq!(hosts(&hops), ["local@ssh.example.com:22"]);
    assert_eq!(methods(&hops[0]), ["keyboard-interactive"]);
}

#[test]
fn a_config_alias_brings_its_jump_chain_and_settings() {
    let password = SshAuth::Password(Some(SecretString::from("synthetic")));
    let hops = plan(&spec("db-box", password), CONFIG).expect("route");
    assert_eq!(
        hosts(&hops),
        ["ops@bastion.example.com:2222", "app@10.0.0.7:22"]
    );
    assert_eq!(
        methods(&hops[0]),
        [
            "agent",
            "key:/home/me/.ssh/bastion_ed25519",
            "keyboard-interactive"
        ]
    );
    assert_eq!(methods(&hops[1]), ["password", "keyboard-interactive"]);
}

#[test]
fn profile_fields_override_the_alias() {
    let route = SshRouteSpec {
        port: Some(2200),
        user: Some(" deploy ".to_owned()),
        jump_hosts: JumpSpec::parse_list("edge@jump.example.com:23").expect("jumps"),
        ..spec(
            "db-box",
            SshAuth::PrivateKey {
                path: PathBuf::from("~/.ssh/deploy"),
                passphrase: None,
            },
        )
    };
    let hops = plan(&route, CONFIG).expect("route");
    assert_eq!(
        hosts(&hops),
        ["edge@jump.example.com:23", "deploy@10.0.0.7:2200"]
    );
    assert_eq!(
        methods(&hops[0]),
        ["agent", "key:/home/me/.ssh/deploy", "keyboard-interactive"],
        "jump hosts also try the profile's key"
    );
    assert_eq!(
        methods(&hops[1]),
        ["key:/home/me/.ssh/deploy", "keyboard-interactive"]
    );
}

#[test]
fn agent_auth_asks_the_agent_first_then_the_alias_keys() {
    let hops = plan(&spec("db-box", SshAuth::Agent), CONFIG).expect("route");
    assert_eq!(
        methods(&hops[1]),
        [
            "agent",
            "key:/home/me/.ssh/app_ed25519",
            "keyboard-interactive"
        ],
        "an explicit agent choice still asks the agent first"
    );
    let direct = plan(&spec("plain", SshAuth::Agent), "").expect("route");
    assert_eq!(methods(&direct[0]), ["agent", "keyboard-interactive"]);
}

#[test]
fn keyboard_interactive_only_and_bad_routes() {
    let hops = plan(&spec("h", SshAuth::KeyboardInteractive), "").expect("route");
    assert_eq!(methods(&hops[0]), ["keyboard-interactive"]);
    assert_eq!(
        plan(&spec("  ", SshAuth::Agent), "").err(),
        Some(RouteError::MissingHost)
    );
    let looping = "Host a\n  ProxyJump b\nHost b\n  ProxyJump a\n";
    assert!(matches!(
        plan(&spec("a", SshAuth::Agent), looping),
        Err(RouteError::TooManyHops { .. })
    ));
}
