// SPDX-License-Identifier: Apache-2.0
//! OpenSSH container matrix (docs/17 §2.2, docs/24 §10). Every key, password
//! and agent is generated at test time; tests skip when Docker is unavailable
//! (`OPENROWS_REQUIRE_DOCKER=1`, used by `just test-ssh`, turns that into a failure).

mod auth;
mod host_keys_and_jumps;
mod route;
mod support;

use openrows_net::{AuthMethod, SshHop};
use support::{Container, ContainerSpec};

/// An sshd with port 22 published on an ephemeral loopback port.
fn published<'a>(password: &'a str, authorized_keys: &'a str) -> ContainerSpec<'a> {
    ContainerSpec {
        password,
        authorized_keys,
        publish: true,
        network: None,
    }
}

/// A direct hop to a published container.
fn hop(server: &Container, user: &str, auth: Vec<AuthMethod>) -> SshHop {
    SshHop {
        host: "127.0.0.1".into(),
        port: server.port(),
        user: user.into(),
        auth,
    }
}
