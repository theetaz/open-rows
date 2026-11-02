// SPDX-License-Identifier: Apache-2.0
//! Public metadata excludes credentials; secret maps are separately keychain-owned.
use super::ClientError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

/// User-authorised endpoint; never discovered from project files.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Endpoint {
    /// Spawn a direct executable, without a shell or inherited environment.
    Stdio {
        /// Public executable path/name, never a shell command.
        command: String,
        /// Public non-secret arguments.
        args: Vec<String>,
    },
    /// HTTPS, or explicit loopback HTTP, without URL credentials.
    Http {
        /// Validated non-secret endpoint.
        url: String,
    },
}
/// Non-secret device-local server preferences. Tools default disabled.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServerConfig {
    /// Validated stable identifier.
    pub id: String,
    /// Display-only user label.
    pub name: String,
    /// Explicit transport.
    pub endpoint: Endpoint,
    /// Exact tool names the user enabled.
    pub enabled_tools: Vec<String>,
}
impl ServerConfig {
    /// Validate bounds and reject credential-bearing URLs.
    /// # Errors
    /// Fixed invalid-configuration error.
    pub fn validate(&self) -> Result<(), ClientError> {
        openrows_ai::validate_id(&self.id).map_err(|_| ClientError::Invalid)?;
        if !clean(&self.name, 128) || self.enabled_tools.len() > 64 {
            return Err(ClientError::Invalid);
        }
        let mut names = HashSet::new();
        if self
            .enabled_tools
            .iter()
            .any(|name| !tool_name(name) || !names.insert(name))
        {
            return Err(ClientError::Invalid);
        }
        match &self.endpoint {
            Endpoint::Stdio { command, args } => {
                if !clean(command, 4096)
                    || args.len() > 32
                    || args
                        .iter()
                        .any(|arg| arg.len() > 4096 || arg.chars().any(char::is_control))
                {
                    return Err(ClientError::Invalid);
                }
            }
            Endpoint::Http { url } => validate_url(url)?,
        }
        Ok(())
    }
}
fn clean(value: &str, limit: usize) -> bool {
    !value.trim().is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
}
pub(super) fn tool_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}
fn validate_url(value: &str) -> Result<(), ClientError> {
    let url = url::Url::parse(value).map_err(|_| ClientError::Invalid)?;
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if value.len() > 2048
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !(url.scheme() == "https" || url.scheme() == "http" && local)
    {
        return Err(ClientError::Invalid);
    }
    Ok(())
}
/// Stored only through the OS keychain; Debug deliberately exposes nothing.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Credentials {
    /// Private child environment.
    pub env: BTreeMap<String, String>,
    /// Private HTTP headers.
    pub headers: BTreeMap<String, String>,
}
impl Credentials {
    /// Bound secret inputs and protect MCP framing headers.
    /// # Errors
    /// Fixed invalid-configuration error, without values.
    pub fn validate(&self) -> Result<(), ClientError> {
        if self.env.len() > 32 || self.headers.len() > 32 {
            return Err(ClientError::Invalid);
        }
        for (key, value) in &self.env {
            if key.is_empty()
                || key.len() > 128
                || key.contains(['=', '\0'])
                || value.len() > 4096
                || value.contains('\0')
            {
                return Err(ClientError::Invalid);
            }
        }
        for (key, value) in &self.headers {
            if value.len() > 4096
                || reqwest::header::HeaderName::from_bytes(key.as_bytes()).is_err()
                || reqwest::header::HeaderValue::from_str(value).is_err()
                || matches!(
                    key.to_ascii_lowercase().as_str(),
                    "host"
                        | "content-length"
                        | "content-type"
                        | "accept"
                        | "mcp-session-id"
                        | "mcp-protocol-version"
                )
            {
                return Err(ClientError::Invalid);
            }
        }
        Ok(())
    }
}
impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Credentials([redacted])")
    }
}
