// SPDX-License-Identifier: Apache-2.0
//! An optional SOCKS5 or HTTP `CONNECT` proxy as the first hop of a route
//! (docs/24 §2). The proxy password is a keychain item
//! ([`super::SecretSlot::ProxyPassword`]).

use serde::{Deserialize, Serialize};

use super::ProfileError;

/// Proxy protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProxyKind {
    /// SOCKS5; host names are resolved by the proxy.
    Socks5,
    /// HTTP `CONNECT` tunnel.
    Http,
}

/// A proxy in front of the database or of the first SSH hop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxySettings {
    /// Protocol.
    pub kind: ProxyKind,
    /// Proxy host name or IP address.
    pub host: String,
    /// Proxy port.
    pub port: u16,
    /// Login user; `None` for anonymous proxies (no password is used then).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

impl ProxySettings {
    /// Check required fields.
    ///
    /// # Errors
    /// [`ProfileError::Invalid`] naming the first bad field.
    pub fn validate(&self) -> Result<(), ProfileError> {
        let invalid = |field, reason| Err(ProfileError::Invalid { field, reason });
        if self.host.trim().is_empty() {
            return invalid("proxy.host", "must not be empty");
        }
        if self.port == 0 {
            return invalid("proxy.port", "must be 1-65535");
        }
        if self
            .user
            .as_ref()
            .is_some_and(|user| user.trim().is_empty())
        {
            return invalid("proxy.user", "must not be blank");
        }
        Ok(())
    }
}
