// SPDX-License-Identifier: Apache-2.0
//! The on-disk connection profile model (docs/15 §3). Contains no secrets.

use std::{collections::BTreeMap, fmt, str::FromStr};

use openrows_secrets::SecretKey;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{EnvKind, EnvTag, ProfileError, Safety};

/// Newest profile file format version (`"openrows": 1`).
pub const PROFILE_FORMAT: u32 = 1;

/// Default `PostgreSQL` port.
const DEFAULT_PORT: u16 = 5432;

/// Stable UUID v7 identifier of a connection profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProfileId(Uuid);

impl ProfileId {
    /// A fresh, time-ordered id.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for ProfileId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ProfileId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for ProfileId {
    type Err = ProfileError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(text)
            .map(Self)
            .map_err(|_| ProfileError::Invalid {
                field: "id",
                reason: "not a UUID",
            })
    }
}

/// Database driver of a profile. Only `PostgreSQL` exists so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Driver {
    /// `PostgreSQL` and wire-compatible servers.
    #[default]
    Postgres,
}

/// libpq `sslmode` values (docs/12 §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TlsMode {
    /// Never use TLS.
    Disable,
    /// Try plaintext first, then TLS.
    Allow,
    /// Try TLS first, then plaintext.
    Prefer,
    /// TLS without certificate verification.
    Require,
    /// TLS, verify the CA chain.
    VerifyCa,
    /// TLS, verify the CA chain and host name.
    VerifyFull,
}

impl TlsMode {
    /// `verify-full` for remote hosts, `prefer` for loopback and sockets.
    #[must_use]
    pub fn default_for_host(host: &str) -> Self {
        let local = matches!(host, "localhost" | "127.0.0.1" | "::1") || host.starts_with('/');
        if local {
            Self::Prefer
        } else {
            Self::VerifyFull
        }
    }

    /// Parse a libpq `sslmode` string.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "disable" => Self::Disable,
            "allow" => Self::Allow,
            "prefer" => Self::Prefer,
            "require" => Self::Require,
            "verify-ca" => Self::VerifyCa,
            "verify-full" => Self::VerifyFull,
            _ => return None,
        })
    }
}

/// TLS settings; certificates and the skip-verify toggle come with M1 TLS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TlsSettings {
    /// Negotiation and verification mode.
    pub mode: TlsMode,
}

/// `PostgreSQL` connection parameters (everything except secrets).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PgParams {
    /// Host name, IP literal (IPv6 without brackets) or socket directory.
    pub host: String,
    /// TCP port.
    pub port: u16,
    /// Database name; the server defaults it to the user name when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    /// Login role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// `application_name` reported to the server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_name: Option<String>,
    /// Other libpq parameters (e.g. `connect_timeout`), passed through.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, String>,
}

impl Default for PgParams {
    fn default() -> Self {
        Self {
            host: "localhost".to_owned(),
            port: DEFAULT_PORT,
            database: None,
            user: None,
            application_name: None,
            options: BTreeMap::new(),
        }
    }
}

/// Where a secret is kept; the value itself never appears in the profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecretRef {
    /// OS keychain item `conn/<profileId>/<name>` (docs/12 §3).
    Keychain,
}

/// Secret references of a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SecretRefs {
    /// Database password, if one is stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<SecretRef>,
}

/// A personal connection profile (`connections/<id>.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionProfile {
    /// File format version, [`PROFILE_FORMAT`].
    pub openrows: u32,
    /// Stable id; also names the file and keychain accounts.
    pub id: ProfileId,
    /// Display name.
    pub name: String,
    /// Database driver.
    pub driver: Driver,
    /// Sidebar group; `None` means ungrouped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Environment tag.
    pub env_tag: EnvTag,
    /// Connection parameters.
    pub params: PgParams,
    /// Secret references (never values).
    #[serde(default)]
    pub secrets: SecretRefs,
    /// TLS settings.
    pub tls: TlsSettings,
    /// Write-safety switches.
    pub safety: Safety,
}

impl ConnectionProfile {
    /// A new `PostgreSQL` profile with a fresh id, env-based safety defaults
    /// and a host-based TLS default.
    #[must_use]
    pub fn new_postgres(name: &str, params: PgParams, env_tag: EnvTag) -> Self {
        Self {
            openrows: PROFILE_FORMAT,
            id: ProfileId::new(),
            name: name.to_owned(),
            driver: Driver::Postgres,
            group: None,
            safety: Safety::default_for(env_tag.kind),
            env_tag,
            tls: TlsSettings {
                mode: TlsMode::default_for_host(&params.host),
            },
            params,
            secrets: SecretRefs::default(),
        }
    }

    /// Change the environment tag; moving to prod turns on confirm-writes.
    pub fn set_env_tag(&mut self, env_tag: EnvTag) {
        if env_tag.kind == EnvKind::Prod {
            self.safety.confirm_writes = true;
        }
        self.env_tag = env_tag;
    }

    /// Keychain key of this profile's password.
    #[must_use]
    pub fn password_key(&self) -> SecretKey {
        SecretKey::password(&self.id.to_string())
    }

    /// Check required fields.
    ///
    /// # Errors
    /// [`ProfileError::Invalid`] naming the first bad field.
    pub fn validate(&self) -> Result<(), ProfileError> {
        let invalid = |field, reason| Err(ProfileError::Invalid { field, reason });
        if self.name.trim().is_empty() {
            return invalid("name", "must not be empty");
        }
        if self.params.host.trim().is_empty() {
            return invalid("params.host", "must not be empty");
        }
        if self.params.port == 0 {
            return invalid("params.port", "must be 1-65535");
        }
        Ok(())
    }
}
