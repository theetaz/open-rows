// SPDX-License-Identifier: Apache-2.0
//! Webview-facing connection DTOs and their mapping to store types.
//!
//! Secrets only travel webview → core ([`PasswordInput`]); nothing here sends
//! a password back.
use openrows_store::profile::{ConnectionProfile, EnvKind, EnvTag, ParsedUrl, PgParams, Safety};
use serde::{Deserialize, Serialize};
use specta::Type;

pub(crate) use super::enums::{EnvKindDto, TlsModeDto};
use super::error::IpcError;

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
    host: String,
    port: u16,
    database: Option<String>,
    user: Option<String>,
    tls_mode: TlsModeDto,
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
            host: profile.params.host.clone(),
            port: profile.params.port,
            database: profile.params.database.clone(),
            user: profile.params.user.clone(),
            tls_mode: profile.tls.mode.into(),
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

/// What to do with the password on save or test; the only secret path.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum PasswordInput {
    /// Use the stored password (none for a new profile).
    Keep,
    /// No password.
    Clear,
    /// A typed password.
    Set { value: String },
    /// The password inside a pasted `postgres://` URL.
    FromUrl { url: String },
}

/// Connection form values sent on save and test.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfileInput {
    pub(crate) name: String,
    pub(crate) group: Option<String>,
    pub(crate) env: EnvInput,
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) database: Option<String>,
    pub(crate) user: Option<String>,
    pub(crate) tls_mode: TlsModeDto,
    pub(crate) safety: SafetyDto,
    pub(crate) password: PasswordInput,
}

impl ProfileInput {
    /// Connection parameters with blank optional fields removed.
    pub(crate) fn params(&self) -> PgParams {
        PgParams {
            host: self.host.trim().to_owned(),
            port: self.port,
            database: non_blank(self.database.as_deref()),
            user: non_blank(self.user.as_deref()),
            ..PgParams::default()
        }
    }

    /// Copy the form values onto `profile`, keeping fields the form does not
    /// edit (application name, extra options).
    ///
    /// # Errors
    /// `Invalid` for a custom environment without a label or with a bad colour.
    pub(crate) fn apply(&self, profile: &mut ConnectionProfile) -> Result<(), IpcError> {
        let params = self.params();
        self.name.trim().clone_into(&mut profile.name);
        profile.group = non_blank(self.group.as_deref());
        profile.env_tag = env_tag(&self.env)?;
        profile.params.host = params.host;
        profile.params.port = params.port;
        profile.params.database = params.database;
        profile.params.user = params.user;
        profile.tls.mode = self.tls_mode.into();
        profile.safety = self.safety.into();
        Ok(())
    }
}

/// Form fields filled from a pasted URL; only says whether it had a password.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UrlDraft {
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
            host: parsed.params.host,
            port: parsed.params.port,
            database: parsed.params.database,
            user: parsed.params.user,
            tls_mode: parsed.tls_mode.into(),
            password_provided: parsed.password.is_some(),
        }
    }
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
pub(crate) mod tests {
    use super::*;
    use openrows_store::profile::parse_postgres_url;

    pub(crate) fn input(env: EnvKindDto) -> ProfileInput {
        ProfileInput {
            name: " Local PG ".to_owned(),
            group: Some("  ".to_owned()),
            env: EnvInput {
                kind: env,
                label: None,
                color: None,
            },
            host: " localhost ".to_owned(),
            port: 5432,
            database: Some(String::new()),
            user: Some("app".to_owned()),
            tls_mode: TlsModeDto::Prefer,
            safety: SafetyDto {
                read_only: false,
                confirm_writes: true,
            },
            password: PasswordInput::Keep,
        }
    }

    #[test]
    fn apply_trims_fields_and_drops_blank_options() -> Result<(), IpcError> {
        let mut profile =
            ConnectionProfile::new_postgres("x", PgParams::default(), EnvTag::new(EnvKind::Dev));
        input(EnvKindDto::Staging).apply(&mut profile)?;
        assert_eq!(profile.name, "Local PG");
        assert_eq!(profile.group, None);
        assert_eq!(profile.params.host, "localhost");
        assert_eq!(profile.params.database, None);
        assert_eq!(profile.params.user.as_deref(), Some("app"));
        assert_eq!(profile.env_tag.kind, EnvKind::Staging);
        assert!(profile.safety.confirm_writes);
        Ok(())
    }

    #[test]
    fn custom_environment_needs_a_label_and_a_hex_colour() {
        let mut env = EnvInput {
            kind: EnvKindDto::Custom,
            label: None,
            color: None,
        };
        assert!(env_tag(&env).is_err());
        env.label = Some("qa".to_owned());
        assert_eq!(env_tag(&env).map(|tag| tag.color.len()), Ok(7));
        env.color = Some("red".to_owned());
        assert!(env_tag(&env).is_err());
        env.color = Some("#0a0B0c".to_owned());
        assert_eq!(env_tag(&env).map(|tag| tag.color), Ok("#0a0B0c".to_owned()));
    }

    #[test]
    fn url_draft_reports_but_never_contains_the_password() -> Result<(), Box<dyn std::error::Error>>
    {
        let parsed = parse_postgres_url("postgres://app:s3cret-synthetic@db.test:6543/orders")?;
        let json = serde_json::to_string(&UrlDraft::from(parsed))?;
        assert!(!json.contains("s3cret-synthetic"));
        assert!(json.contains("\"passwordProvided\":true"));
        assert!(json.contains("\"port\":6543"));
        Ok(())
    }

    #[test]
    fn profile_dto_has_no_secret_fields() -> Result<(), serde_json::Error> {
        let profile =
            ConnectionProfile::new_postgres("p", PgParams::default(), EnvTag::new(EnvKind::Prod));
        let json = serde_json::to_value(ProfileDto::from(&profile))?;
        assert_eq!(json["hasPassword"], false);
        assert_eq!(json["safety"]["confirmWrites"], true);
        assert_eq!(json["env"]["kind"], "prod");
        assert!(json.get("password").is_none());
        Ok(())
    }
}
