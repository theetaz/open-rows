// SPDX-License-Identifier: Apache-2.0
//! MCP servers declared by a Project's `.openrows/project.yaml` (docs/23 §3.2).
//! The file names environment variables and HTTP headers but cannot hold
//! their values: those are entered by the user and kept in the OS keychain.
use super::{ClientError, Credentials, Endpoint, ServerConfig, config::tool_name};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::HashSet;

/// Most servers one Project may declare.
pub const PROJECT_SERVER_LIMIT: usize = 8;
/// Longest declared server id, so the device key stays a valid identifier.
const ID_LIMIT: usize = 48;

/// One shared server definition. Unknown fields, and so any value map, are refused.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectServer {
    /// Identifier unique within the Project.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Exact transport the user reviews before trusting it.
    pub endpoint: Endpoint,
    /// Names of environment variables the user supplies (stdio only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<String>,
    /// Names of HTTP headers the user supplies (HTTP only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<String>,
}

impl ProjectServer {
    /// Validate the transport like a user server and every declared name.
    /// # Errors
    /// Fixed invalid-configuration error.
    pub fn validate(&self) -> Result<(), ClientError> {
        if self.id.len() > ID_LIMIT || self.name.len() > 80 {
            return Err(ClientError::Invalid);
        }
        self.effective("validate", "Project", Vec::new())
            .validate()?;
        let stdio = matches!(self.endpoint, Endpoint::Stdio { .. });
        if (stdio && !self.headers.is_empty()) || (!stdio && !self.env.is_empty()) {
            return Err(ClientError::Invalid);
        }
        unique(&self.env, env_name)?;
        unique(&self.headers, header_name)
    }

    /// SHA-256 of the whole definition. Trust is bound to it, so any change to
    /// the command, arguments, URL, names or label needs a fresh review.
    #[must_use]
    pub fn digest(&self) -> String {
        // Length-delimited JSON leaves no ambiguous field or argument boundary.
        let bytes = serde_json::to_vec(self).unwrap_or_default();
        format!("{:x}", Sha256::digest(bytes))
    }

    /// The runnable configuration under its device key, shown as `project: name`.
    #[must_use]
    pub fn effective(&self, key: &str, project: &str, enabled_tools: Vec<String>) -> ServerConfig {
        ServerConfig {
            id: key.to_owned(),
            name: format!("{project}: {}", self.name),
            endpoint: self.endpoint.clone(),
            enabled_tools,
        }
    }

    /// Whether `name` is a variable or header this definition declares.
    #[must_use]
    pub fn declares(&self, name: &str) -> bool {
        self.env
            .iter()
            .chain(&self.headers)
            .any(|item| item == name)
    }

    /// Keep only the declared names, so stale or foreign values never reach the server.
    #[must_use]
    pub fn restrict(&self, mut credentials: Credentials) -> Credentials {
        credentials.env.retain(|name, _| self.env.contains(name));
        credentials
            .headers
            .retain(|name, _| self.headers.contains(name));
        credentials
    }
}

/// Validate a Project's list: bounded, unique ids, each definition valid.
/// # Errors
/// Fixed invalid-configuration error.
pub fn validate_project_servers(servers: &[ProjectServer]) -> Result<(), ClientError> {
    if servers.len() > PROJECT_SERVER_LIMIT {
        return Err(ClientError::Invalid);
    }
    let mut ids = HashSet::new();
    for server in servers {
        server.validate()?;
        if !ids.insert(&server.id) {
            return Err(ClientError::Invalid);
        }
    }
    Ok(())
}

/// Device key of one Project server: a digest of the Project root plus its id.
/// It addresses the keychain entry, catalogue and trust record.
#[must_use]
pub fn project_key(root: &str, id: &str) -> String {
    let digest = format!("{:x}", Sha256::digest(root.as_bytes()));
    format!("p{}-{id}", &digest[..12])
}

fn unique(names: &[String], valid: fn(&str) -> bool) -> Result<(), ClientError> {
    let mut seen = HashSet::new();
    if names.len() > 32 || names.iter().any(|name| !valid(name) || !seen.insert(name)) {
        return Err(ClientError::Invalid);
    }
    Ok(())
}

fn env_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    name.len() <= 128
        && bytes
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn header_name(name: &str) -> bool {
    tool_name(name)
        && !matches!(
            name.to_ascii_lowercase().as_str(),
            "host"
                | "content-length"
                | "content-type"
                | "accept"
                | "mcp-session-id"
                | "mcp-protocol-version"
        )
}

#[cfg(test)]
#[path = "project_tests.rs"]
pub(super) mod tests;
