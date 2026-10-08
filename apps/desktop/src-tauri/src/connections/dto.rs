// SPDX-License-Identifier: Apache-2.0
//! Webview-facing connection DTOs and their mapping to store types.
//!
//! Secrets only travel webview → core ([`PasswordInput`]); nothing here sends
//! a password back.
use openrows_secrets::SecretString;
use openrows_store::profile::{
    ConnectionProfile, EnvKind, EnvTag, ParsedUrl, PasswordChange, Safety, SecretChanges,
    ServerParams, TlsMode, parse_url,
};
use serde::{Deserialize, Serialize};
use specta::Type;

pub(crate) use super::engine_dto::{EngineDto, FileDto};
pub(crate) use super::enums::{EnvKindDto, TlsModeDto};
use super::error::IpcError;
use super::route_dto::{
    SshDto, SshInput, TlsFilesDto, TlsFilesInput, apply_route, ssh_dto, tls_files_dto,
};

/// Environment tag as shown in the rail, header and toolbar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub(crate) struct EnvDto {
    kind: EnvKindDto,
    label: Option<String>,
    /// `#rrggbb`; the UI uses theme tokens for the four standard kinds.
    color: String,
}

/// Write-safety switches (FR-CONN-07).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SafetyDto {
    read_only: bool,
    confirm_writes: bool,
}

impl From<SafetyDto> for Safety {
    fn from(safety: SafetyDto) -> Self {
        Self {
            read_only: safety.read_only,
            confirm_writes: safety.confirm_writes,
        }
    }
}

/// A saved connection without secrets; `hasPassword` says whether the
/// keychain holds one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfileDto {
    id: String,
    name: String,
    group: Option<String>,
    env: EnvDto,
    engine: EngineDto,
    /// The database file of a file engine.
    file: Option<FileDto>,
    host: String,
    port: u16,
    database: Option<String>,
    user: Option<String>,
    tls_mode: TlsModeDto,
    tls_files: TlsFilesDto,
    ssh: Option<SshDto>,
    has_password: bool,
    safety: SafetyDto,
}

impl From<&ConnectionProfile> for ProfileDto {
    fn from(profile: &ConnectionProfile) -> Self {
        Self {
            id: profile.id.to_string(),
            name: profile.name.clone(),
            group: profile.group.clone(),
            env: EnvDto {
                kind: profile.env_tag.kind.into(),
                label: profile.env_tag.label.clone(),
                color: profile.env_tag.color.clone(),
            },
            engine: profile.driver.into(),
            file: profile.file.as_ref().map(FileDto::from),
            host: profile.params.host.clone(),
            port: profile.params.port,
            database: profile.params.database.clone(),
            user: profile.params.user.clone(),
            tls_mode: profile.tls.mode.into(),
            tls_files: tls_files_dto(profile),
            ssh: ssh_dto(profile),
            has_password: profile.secrets.password.is_some(),
            safety: SafetyDto {
                read_only: profile.safety.read_only,
                confirm_writes: profile.safety.confirm_writes,
            },
        }
    }
}

/// Saved profiles plus files that could not be read.
#[derive(Clone, Debug, Serialize, Type)]
pub(crate) struct ProfileList {
    pub(crate) profiles: Vec<ProfileDto>,
    /// One message per unreadable profile file.
    pub(crate) unreadable: Vec<String>,
}

/// Environment chosen in the form; `color` only matters for `custom`.
#[derive(Clone, Debug, Deserialize, Type)]
pub(crate) struct EnvInput {
    kind: EnvKindDto,
    label: Option<String>,
    color: Option<String>,
}

/// What to do with the password on save or test; `Debug` redacts secrets.
#[derive(Clone, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum PasswordInput {
    /// Use the stored password (none for a new profile).
    Keep,
    /// No password.
    Clear,
    /// A typed password.
    Set { value: String },
    /// The password inside a pasted connection URL.
    FromUrl { url: String },
}

impl std::fmt::Debug for PasswordInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Keep => "Keep",
            Self::Clear => "Clear",
            Self::Set { .. } => "Set([redacted])",
            Self::FromUrl { .. } => "FromUrl([redacted])",
        })
    }
}

/// Connection form values sent on save and test.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfileInput {
    pub(crate) name: String,
    pub(crate) group: Option<String>,
    pub(crate) env: EnvInput,
    /// Absent in forms written before D-12: `PostgreSQL`.
    #[serde(default)]
    pub(crate) engine: EngineDto,
    /// The database file of a file engine.
    #[serde(default)]
    pub(crate) file: Option<FileDto>,
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) database: Option<String>,
    pub(crate) user: Option<String>,
    pub(crate) tls_mode: TlsModeDto,
    /// CA and client certificate files (absent: none).
    #[serde(default)]
    pub(crate) tls_files: TlsFilesInput,
    /// The SSH tunnel; absent or null for a direct connection.
    #[serde(default)]
    pub(crate) ssh: Option<SshInput>,
    pub(crate) safety: SafetyDto,
    pub(crate) password: PasswordInput,
}

impl ProfileInput {
    /// Connection parameters with blank optional fields removed.
    pub(crate) fn params(&self) -> ServerParams {
        ServerParams {
            host: self.host.trim().to_owned(),
            port: self.port,
            database: non_blank(self.database.as_deref()),
            user: non_blank(self.user.as_deref()),
            ..ServerParams::default()
        }
    }

    /// A new profile for the form's engine (fields applied by [`Self::apply`]).
    pub(crate) fn new_profile(&self) -> ConnectionProfile {
        ConnectionProfile::new(
            &self.name,
            self.engine.into(),
            self.params(),
            EnvTag::new(EnvKind::Local),
        )
    }

    /// Copy the form values onto `profile`, keeping fields the form does not
    /// edit (application name, extra options). File engines get their file
    /// and never a route (no SSH, no TLS).
    ///
    /// # Errors
    /// `Invalid` for a custom environment without a label or with a bad colour.
    pub(crate) fn apply(&self, profile: &mut ConnectionProfile) -> Result<(), IpcError> {
        let params = self.params();
        profile.driver = self.engine.into();
        self.name.trim().clone_into(&mut profile.name);
        profile.group = non_blank(self.group.as_deref());
        profile.env_tag = env_tag(&self.env)?;
        profile.params.host = params.host;
        profile.params.port = params.port;
        profile.params.database = params.database;
        profile.params.user = params.user;
        profile.tls.mode = self.tls_mode.into();
        apply_route(&self.tls_files, self.ssh.as_ref(), profile);
        profile.file = self.file.as_ref().map(Into::into);
        if profile.driver.is_file() {
            profile.params = ServerParams::default();
            profile.ssh = None;
            profile.tls.mode = TlsMode::Disable;
        }
        profile.safety = self.safety.into();
        Ok(())
    }

    /// The keychain edits this form asks for.
    ///
    /// # Errors
    /// `Url` when the password comes from an invalid pasted URL.
    pub(crate) fn secret_changes(&self) -> Result<SecretChanges, IpcError> {
        let ssh = self.ssh.as_ref();
        Ok(SecretChanges {
            password: password_change(&self.password)?,
            ssh_password: ssh.map_or(PasswordChange::Keep, |ssh| ssh.password.change()),
            ssh_passphrase: ssh.map_or(PasswordChange::Keep, |ssh| ssh.passphrase.change()),
            tls_key_passphrase: self.tls_files.key_passphrase.change(),
        })
    }
}

/// Form fields filled from a pasted URL; only says whether it had a password.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UrlDraft {
    engine: EngineDto,
    file: Option<FileDto>,
    host: String,
    port: u16,
    database: Option<String>,
    user: Option<String>,
    tls_mode: TlsModeDto,
    password_provided: bool,
}

impl From<ParsedUrl> for UrlDraft {
    fn from(parsed: ParsedUrl) -> Self {
        Self {
            engine: parsed.driver.into(),
            file: parsed.file.as_ref().map(FileDto::from),
            host: parsed.params.host,
            port: parsed.params.port,
            database: parsed.params.database,
            user: parsed.params.user,
            tls_mode: parsed.tls_mode.into(),
            password_provided: parsed.password.is_some(),
        }
    }
}

/// The database password change `input` asks for.
pub(crate) fn password_change(input: &PasswordInput) -> Result<PasswordChange, IpcError> {
    Ok(match input {
        PasswordInput::Keep => PasswordChange::Keep,
        PasswordInput::Clear => PasswordChange::Clear,
        PasswordInput::Set { value } => PasswordChange::Set(SecretString::from(value.as_str())),
        PasswordInput::FromUrl { url } => parse_url(url)?
            .password
            .map_or(PasswordChange::Keep, PasswordChange::Set),
    })
}

fn env_tag(input: &EnvInput) -> Result<EnvTag, IpcError> {
    if input.kind != EnvKindDto::Custom {
        return Ok(EnvTag::new(input.kind.into()));
    }
    let label = non_blank(input.label.as_deref())
        .ok_or_else(|| IpcError::invalid("Custom environment needs a label"))?;
    let default = EnvTag::new(EnvKind::Custom).color;
    let color = input.color.clone().unwrap_or(default);
    if !is_hex_colour(&color) {
        return Err(IpcError::invalid("Environment colour must be #rrggbb"));
    }
    Ok(EnvTag::custom(&label, &color))
}

fn is_hex_colour(text: &str) -> bool {
    text.len() == 7 && text.starts_with('#') && text[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn non_blank(text: Option<&str>) -> Option<String> {
    text.map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
pub(crate) mod tests;
