// SPDX-License-Identifier: Apache-2.0
//! Webview DTOs for a profile's proxy (M1-06) and the network layer's view
//! of it. The proxy password only travels webview → core.
use std::sync::Arc;

use openrows_net::{ProxyAuth, ProxyKind as NetProxyKind, ProxySpec, SecretString};
use openrows_secrets::SecretString as StoredSecret;
use openrows_store::profile::{ConnectionProfile, ProxyKind, ProxySettings, SecretSlot};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::route_dto::SecretInput;

/// Proxy protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ProxyKindDto {
    Socks5,
    Http,
}

impl From<ProxyKind> for ProxyKindDto {
    fn from(kind: ProxyKind) -> Self {
        match kind {
            ProxyKind::Socks5 => Self::Socks5,
            ProxyKind::Http => Self::Http,
        }
    }
}

impl From<ProxyKindDto> for ProxyKind {
    fn from(kind: ProxyKindDto) -> Self {
        match kind {
            ProxyKindDto::Socks5 => Self::Socks5,
            ProxyKindDto::Http => Self::Http,
        }
    }
}

/// A saved proxy; says whether its password is stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProxyDto {
    kind: ProxyKindDto,
    host: String,
    port: u16,
    user: Option<String>,
    has_password: bool,
}

/// Proxy fields from the form.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProxyInput {
    pub(crate) kind: ProxyKindDto,
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) user: Option<String>,
    pub(crate) password: SecretInput,
}

pub(crate) fn proxy_dto(profile: &ConnectionProfile) -> Option<ProxyDto> {
    let proxy = profile.proxy.as_ref()?;
    Some(ProxyDto {
        kind: proxy.kind.into(),
        host: proxy.host.clone(),
        port: proxy.port,
        user: proxy.user.clone(),
        has_password: profile.secrets.get(SecretSlot::ProxyPassword).is_some(),
    })
}

/// The store settings for the form's proxy (trimmed; a blank user is none).
pub(crate) fn proxy_settings(input: &ProxyInput) -> ProxySettings {
    ProxySettings {
        kind: input.kind.into(),
        host: input.host.trim().to_owned(),
        port: input.port,
        user: input
            .user
            .as_deref()
            .map(str::trim)
            .filter(|user| !user.is_empty())
            .map(str::to_owned),
    }
}

/// The network layer's proxy for a profile, with its keychain password.
pub(crate) fn proxy_spec(
    settings: &ProxySettings,
    password: Option<&StoredSecret>,
) -> Arc<ProxySpec> {
    Arc::new(ProxySpec {
        kind: match settings.kind {
            ProxyKind::Socks5 => NetProxyKind::Socks5,
            ProxyKind::Http => NetProxyKind::HttpConnect,
        },
        host: settings.host.clone(),
        port: settings.port,
        auth: settings.user.as_ref().map(|user| ProxyAuth {
            user: user.clone(),
            password: password.map_or_else(SecretString::default, |secret| {
                SecretString::from(secret.expose())
            }),
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(user: Option<&str>) -> ProxyInput {
        ProxyInput {
            kind: ProxyKindDto::Http,
            host: " proxy.corp ".to_owned(),
            port: 3128,
            user: user.map(str::to_owned),
            password: SecretInput::Keep,
        }
    }

    #[test]
    fn form_values_are_trimmed_and_blank_users_dropped() {
        let settings = proxy_settings(&input(Some("  ")));
        assert_eq!(
            (settings.host.as_str(), settings.user),
            ("proxy.corp", None)
        );
        assert_eq!(
            proxy_settings(&input(Some(" ops "))).user.as_deref(),
            Some("ops")
        );
    }

    #[test]
    fn specs_carry_the_keychain_password_only_with_a_user() {
        let password = StoredSecret::from("proxy-pw-synthetic");
        let spec = proxy_spec(&proxy_settings(&input(Some("ops"))), Some(&password));
        assert_eq!(spec.kind, NetProxyKind::HttpConnect);
        let auth = spec.auth.as_ref().map(|auth| auth.password.expose());
        assert_eq!(auth, Some("proxy-pw-synthetic"));
        assert!(!format!("{spec:?}").contains("proxy-pw-synthetic"));
        let anonymous = proxy_spec(&proxy_settings(&input(None)), Some(&password));
        assert!(anonymous.auth.is_none());
    }
}
