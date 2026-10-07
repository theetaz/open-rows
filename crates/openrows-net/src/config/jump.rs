// SPDX-License-Identifier: Apache-2.0
//! `ProxyJump` destination syntax: `[user@]host[:port]`, `[v6]:port`, `ssh://…`.

use super::parse::parse_port;

/// One jump host from a `ProxyJump` list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JumpSpec {
    /// Login user, overriding the jump host's own `User`.
    pub user: Option<String>,
    /// Host name or alias, looked up in the config again.
    pub host: String,
    /// Port, overriding the jump host's own `Port`.
    pub port: Option<u16>,
}

/// Parse a comma-separated `ProxyJump` value.
pub(crate) fn parse_jump_list(value: &str) -> Result<Vec<JumpSpec>, String> {
    value.split(',').map(parse_jump).collect()
}

fn parse_jump(spec: &str) -> Result<JumpSpec, String> {
    let spec = spec.trim();
    let spec = spec.strip_prefix("ssh://").unwrap_or(spec);
    let (user, host_port) = match spec.rsplit_once('@') {
        Some((user, host_port)) => (Some(user), host_port),
        None => (None, spec),
    };
    let (host, port) = split_host_port(host_port)?;
    if host.is_empty() || user.is_some_and(str::is_empty) {
        return Err(format!("invalid ProxyJump destination {spec:?}"));
    }
    Ok(JumpSpec {
        user: user.map(str::to_owned),
        host: host.to_owned(),
        port,
    })
}

fn split_host_port(value: &str) -> Result<(&str, Option<u16>), String> {
    if let Some(rest) = value.strip_prefix('[') {
        let (host, after) = rest
            .split_once(']')
            .ok_or_else(|| format!("unterminated '[' in {value:?}"))?;
        if after.is_empty() {
            return Ok((host, None));
        }
        let port = after
            .strip_prefix(':')
            .ok_or_else(|| format!("unexpected text after ']' in {value:?}"))?;
        return Ok((host, Some(parse_port(port)?)));
    }
    match value.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => Ok((host, Some(parse_port(port)?))),
        _ => Ok((value, None)),
    }
}

#[cfg(test)]
mod tests {
    use super::{JumpSpec, parse_jump_list};

    fn spec(user: Option<&str>, host: &str, port: Option<u16>) -> JumpSpec {
        JumpSpec {
            user: user.map(str::to_owned),
            host: host.to_owned(),
            port,
        }
    }

    #[test]
    fn parses_every_destination_form() {
        let parsed = parse_jump_list("bastion,ops@jump:2222,ssh://me@edge:23,[::1]:2200,fe80::1");
        assert_eq!(
            parsed,
            Ok(vec![
                spec(None, "bastion", None),
                spec(Some("ops"), "jump", Some(2222)),
                spec(Some("me"), "edge", Some(23)),
                spec(None, "::1", Some(2200)),
                spec(None, "fe80::1", None),
            ])
        );
    }

    #[test]
    fn rejects_malformed_destinations() {
        for value in [
            "",
            "@host",
            "host:0",
            "host:99999",
            "[::1",
            "[::1]x",
            "a,,b",
        ] {
            assert!(parse_jump_list(value).is_err(), "{value:?} should fail");
        }
    }
}
