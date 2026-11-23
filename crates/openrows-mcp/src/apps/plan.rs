// SPDX-License-Identifier: Apache-2.0
//! Identity and port choices made before an app is registered.
use super::AgentApp;
use std::net::{Ipv4Addr, TcpListener};

/// First port tried when the server uses an OS-chosen port.
pub const PREFERRED_PORT: u16 = 32123;
/// Longest credential lifetime the token store accepts, less a minute of slack.
pub const APP_TOKEN_SECONDS: u32 = 30 * 86_400 - 60;

/// A fresh random client id such as `claude-3f9a0c1d2b4e`.
#[must_use]
pub fn new_client_id(app: AgentApp) -> String {
    use std::fmt::Write;
    let suffix =
        rand::random::<[u8; 6]>()
            .iter()
            .fold(String::with_capacity(12), |mut text, byte| {
                let _ = write!(text, "{byte:02x}");
                text
            });
    format!("{}-{suffix}", app.id())
}
/// Whether a client id was made for this app by [`new_client_id`].
#[must_use]
pub fn client_matches(app: AgentApp, client: &str) -> bool {
    client
        .strip_prefix(app.id())
        .and_then(|rest| rest.strip_prefix('-'))
        .is_some_and(|rest| rest.len() == 12 && rest.bytes().all(|byte| byte.is_ascii_hexdigit()))
}
/// A port that stays the same across restarts: the configured one, else the
/// running server's if it is in the preferred range, else the first free
/// port from [`PREFERRED_PORT`].
#[must_use]
pub fn stable_port(configured: u16, running: Option<u16>) -> u16 {
    if configured != 0 {
        return configured;
    }
    let range = PREFERRED_PORT..PREFERRED_PORT + 100;
    if let Some(port) = running.filter(|port| range.contains(port)) {
        return port;
    }
    range
        .clone()
        .find(|port| TcpListener::bind((Ipv4Addr::LOCALHOST, *port)).is_ok())
        .unwrap_or(PREFERRED_PORT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_ids_are_random_and_bound_to_their_app() {
        let first = new_client_id(AgentApp::Codex);
        assert!(client_matches(AgentApp::Codex, &first));
        assert!(!client_matches(AgentApp::Claude, &first));
        assert_ne!(first, new_client_id(AgentApp::Codex));
        assert!(!client_matches(AgentApp::Codex, "codex-../../x"));
        assert!(openrows_ai::validate_id(&first).is_ok());
    }

    #[test]
    fn ports_prefer_configuration_then_a_free_stable_port() {
        assert_eq!(stable_port(40000, Some(50000)), 40000);
        assert_eq!(stable_port(0, Some(PREFERRED_PORT + 3)), PREFERRED_PORT + 3);
        let chosen = stable_port(0, Some(61000));
        assert!((PREFERRED_PORT..PREFERRED_PORT + 100).contains(&chosen));
        if let Ok(_held) = TcpListener::bind((Ipv4Addr::LOCALHOST, chosen)) {
            assert_ne!(stable_port(0, None), chosen);
        }
    }
}
