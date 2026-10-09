// SPDX-License-Identifier: Apache-2.0
//! The on-disk connection profile model (docs/15 §3). Contains no secrets.

use std::{fmt, str::FromStr};

use openrows_secrets::SecretKey;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    Driver, EnvKind, EnvTag, FileParams, ProfileError, Safety, ServerParams, SshSettings,
    TlsSettings,
};

/// Newest profile file format version (`"openrows": 1`).
pub const PROFILE_FORMAT: u32 = 1;

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
        if super::network::is_local_host(host) {
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

/// Where a secret is kept; the value itself never appears in the profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecretRef {
    /// OS keychain item `conn/<profileId>/<name>` (docs/12 §3).
    Keychain,
}

/// Secret references of a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretRefs {
    /// Database password, if one is stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<SecretRef>,
    /// SSH password.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_password: Option<SecretRef>,
    /// SSH private key passphrase.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_passphrase: Option<SecretRef>,
    /// TLS client key passphrase.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_key_passphrase: Option<SecretRef>,
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
    /// Database engine (`postgres`, `mysql`, `mariadb`, `sqlite`).
    pub driver: Driver,
    /// Sidebar group; `None` means ungrouped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Environment tag.
    pub env_tag: EnvTag,
    /// Server parameters; kept at their defaults (and ignored) for file engines.
    #[serde(default)]
    pub params: ServerParams,
    /// The database file of a file engine (`SQLite`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<FileParams>,
    /// Secret references (never values).
    #[serde(default)]
    pub secrets: SecretRefs,
    /// TLS settings.
    pub tls: TlsSettings,
    /// SSH tunnel in front of the database; absent for direct connections.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh: Option<SshSettings>,
    /// Write-safety switches.
    pub safety: Safety,
    /// Keep executed statements in the local history (FR-WS-02); on by default.
    /// Files only carry the key when recording is switched off.
    #[serde(
        default = "record_history_default",
        skip_serializing_if = "recording_is_default"
    )]
    pub record_history: bool,
}

/// Profiles written before the switch existed keep recording history.
fn record_history_default() -> bool {
    true
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if passes the field by reference"
)]
fn recording_is_default(record_history: &bool) -> bool {
    *record_history
}

impl ConnectionProfile {
    /// A new server profile with a fresh id, env-based safety defaults and a
    /// host-based TLS default.
    #[must_use]
    pub fn new(name: &str, driver: Driver, params: ServerParams, env_tag: EnvTag) -> Self {
        Self {
            openrows: PROFILE_FORMAT,
            id: ProfileId::new(),
            name: name.to_owned(),
            driver,
            group: None,
            safety: Safety::default_for(env_tag.kind),
            env_tag,
            tls: TlsSettings::new(TlsMode::default_for_host(&params.host)),
            ssh: None,
            params,
            file: None,
            secrets: SecretRefs::default(),
            record_history: true,
        }
    }

    /// A new `PostgreSQL` profile (see [`ConnectionProfile::new`]).
    #[must_use]
    pub fn new_postgres(name: &str, params: ServerParams, env_tag: EnvTag) -> Self {
        Self::new(name, Driver::Postgres, params, env_tag)
    }

    /// A new `SQLite` profile for `file`; TLS does not apply.
    #[must_use]
    pub fn new_file(name: &str, file: FileParams, env_tag: EnvTag) -> Self {
        let mut profile = Self::new(name, Driver::Sqlite, ServerParams::default(), env_tag);
        profile.tls = TlsSettings::new(TlsMode::Disable);
        profile.file = Some(file);
        profile
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

    /// Check required fields: a host and port for servers, a file path and
    /// no SSH tunnel for file engines.
    ///
    /// # Errors
    /// [`ProfileError::Invalid`] naming the first bad field.
    pub fn validate(&self) -> Result<(), ProfileError> {
        let invalid = |field, reason| Err(ProfileError::Invalid { field, reason });
        if self.name.trim().is_empty() {
            return invalid("name", "must not be empty");
        }
        if self.driver.is_file() {
            return self.validate_file();
        }
        if self.params.host.trim().is_empty() {
            return invalid("params.host", "must not be empty");
        }
        if self.params.port == 0 {
            return invalid("params.port", "must be 1-65535");
        }
        self.ssh.as_ref().map_or(Ok(()), SshSettings::validate)
    }

    fn validate_file(&self) -> Result<(), ProfileError> {
        let invalid = |field, reason| Err(ProfileError::Invalid { field, reason });
        let Some(file) = &self.file else {
            return invalid("file.path", "must not be empty");
        };
        if file.path.as_os_str().is_empty() {
            return invalid("file.path", "must not be empty");
        }
        if !file.path.is_absolute() {
            return invalid("file.path", "must be an absolute path");
        }
        if self.ssh.is_some() {
            return invalid("ssh", "local files cannot use an SSH tunnel");
        }
        Ok(())
    }
}
