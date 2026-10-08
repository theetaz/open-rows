// SPDX-License-Identifier: Apache-2.0
//! Expand an alias's `ProxyJump` chain into ordered hops (first hop first).
//!
//! Mirrors OpenSSH: `ProxyJump a,b` connects to `a` (whose own `ProxyJump`
//! applies), then to `b` through `a`, then to the destination through `b`.
//! A jump's `user@host:port` overrides that host's configured user and port.

use crate::config::{HostSettings, JumpSpec, SshConfig};

/// Longest chain accepted; deeper chains are almost certainly loops.
const MAX_HOPS: usize = 8;

/// Failure to expand a route.
#[derive(Debug, thiserror::Error, Eq, PartialEq)]
#[non_exhaustive]
pub enum RouteError {
    /// The chain is longer than the limit, usually a `ProxyJump` loop.
    #[error("ProxyJump chain for {alias} exceeds {max} hops (loop?)")]
    TooManyHops {
        /// Alias whose route was being built.
        alias: String,
        /// Hop limit.
        max: usize,
    },
    /// The route has no SSH server.
    #[error("the SSH host is empty")]
    MissingHost,
    /// A jump host is not `[user@]host[:port]`.
    #[error("jump host: {0}")]
    InvalidJump(String),
}

/// Ordered hops for `alias`: jump hosts first, the destination last.
///
/// # Errors
/// [`RouteError::TooManyHops`] for chains longer than eight hops or loops.
pub fn resolve_route(config: &SshConfig, alias: &str) -> Result<Vec<HostSettings>, RouteError> {
    expand_target(config, config.resolve(alias))
}

/// Ordered hops for already-resolved `target` settings, whose `proxy_jump`
/// may have been overridden by the profile.
pub(crate) fn expand_target(
    config: &SshConfig,
    target: HostSettings,
) -> Result<Vec<HostSettings>, RouteError> {
    let alias = target.alias.clone();
    let hops = expand(config, target, 0, &alias)?;
    if hops.len() > MAX_HOPS {
        return Err(too_many(&alias));
    }
    Ok(hops)
}

fn expand(
    config: &SshConfig,
    target: HostSettings,
    depth: usize,
    alias: &str,
) -> Result<Vec<HostSettings>, RouteError> {
    if depth >= MAX_HOPS {
        return Err(too_many(alias));
    }
    let Some((first, rest)) = target.proxy_jump.split_first() else {
        return Ok(vec![target]);
    };
    let mut hops = expand(config, jump_settings(config, first), depth + 1, alias)?;
    // Later jumps are reached through the previous one; their own ProxyJump is overridden.
    hops.extend(rest.iter().map(|jump| HostSettings {
        proxy_jump: Vec::new(),
        ..jump_settings(config, jump)
    }));
    hops.push(HostSettings {
        proxy_jump: Vec::new(),
        ..target
    });
    Ok(hops)
}

fn jump_settings(config: &SshConfig, jump: &JumpSpec) -> HostSettings {
    let mut settings = config.resolve(&jump.host);
    if let Some(user) = &jump.user {
        settings.user = Some(user.clone());
    }
    if let Some(port) = jump.port {
        settings.port = port;
    }
    settings
}

fn too_many(alias: &str) -> RouteError {
    RouteError::TooManyHops {
        alias: alias.to_owned(),
        max: MAX_HOPS,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{RouteError, resolve_route};
    use crate::config::{FileSystem, SshConfig};

    fn config(text: &str) -> SshConfig {
        SshConfig::parse(
            text,
            Path::new("/cfg"),
            Path::new("/home/fixture"),
            &FileSystem,
        )
        .expect("parses")
    }

    fn summary(text: &str, alias: &str) -> Vec<String> {
        resolve_route(&config(text), alias)
            .expect("route")
            .iter()
            .map(|hop| {
                format!(
                    "{}@{}:{}",
                    hop.user.as_deref().unwrap_or("-"),
                    hop.host_name,
                    hop.port
                )
            })
            .collect()
    }

    #[test]
    fn direct_alias_is_one_hop() {
        assert_eq!(
            summary("Host db\n  HostName 10.0.0.5\n", "db"),
            ["-@10.0.0.5:22"]
        );
    }

    #[test]
    fn jump_chain_uses_each_hosts_config_and_overrides() {
        let text = "Host db\n  HostName 10.0.0.5\n  User app\n  ProxyJump ops@jump1:2022,jump2\n\
                    Host jump1\n  HostName j1.example\n  User ignored\n  ProxyJump edge\n\
                    Host jump2\n  HostName j2.internal\n  ProxyJump never-used\n\
                    Host edge\n  HostName edge.example\n";
        assert_eq!(
            summary(text, "db"),
            [
                "-@edge.example:22",
                "ops@j1.example:2022",
                "-@j2.internal:22",
                "app@10.0.0.5:22"
            ]
        );
    }

    #[test]
    fn loops_are_rejected() {
        let error = resolve_route(
            &config("Host a\n  ProxyJump b\nHost b\n  ProxyJump a\n"),
            "a",
        );
        assert_eq!(
            error,
            Err(RouteError::TooManyHops {
                alias: "a".into(),
                max: 8
            })
        );
    }
}
