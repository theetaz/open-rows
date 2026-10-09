// SPDX-License-Identifier: Apache-2.0
//! Webview DTOs for the engine-agnostic route settings of a profile: the SSH
//! tunnel (D-10) and TLS certificate files (D-11).
//!
//! Secrets only travel webview → core ([`SecretInput`]); the DTOs sent back
//! say whether a secret is stored, never what it is.
use std::path::{Path, PathBuf};

use openrows_store::profile::{
    ConnectionProfile, PasswordChange, SecretSlot, SshAuthMethod, SshSettings,
};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::enums::CloudCaDto;
use super::proxy_dto::{ProxyInput, proxy_settings};

/// What to do with an SSH or TLS secret on save or test. `Debug` redacts
/// the value.
#[derive(Clone, Default, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum SecretInput {
    /// Use the stored secret (none for a new profile).
    #[default]
    Keep,
    /// No secret.
    Clear,
    /// A typed secret.
    Set { value: String },
}

impl std::fmt::Debug for SecretInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Keep => "Keep",
            Self::Clear => "Clear",
            Self::Set { .. } => "Set([redacted])",
        })
    }
}

impl SecretInput {
    pub(crate) fn change(&self) -> PasswordChange {
        match self {
            Self::Keep => PasswordChange::Keep,
            Self::Clear => PasswordChange::Clear,
            Self::Set { value } => PasswordChange::Set(value.as_str().into()),
        }
    }
}

/// SSH authentication method.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "method", rename_all = "camelCase")]
pub(crate) enum SshAuthDto {
    Password,
    #[serde(rename_all = "camelCase")]
    PrivateKey {
        key_file: String,
    },
    Agent,
    KeyboardInteractive,
}

/// A saved SSH tunnel; says whether its password or passphrase is stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SshDto {
    host: String,
    port: Option<u16>,
    user: Option<String>,
    auth: SshAuthDto,
    jump_hosts: Vec<String>,
    has_password: bool,
    has_passphrase: bool,
}

/// SSH tunnel fields from the form.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SshInput {
    pub(crate) host: String,
    pub(crate) port: Option<u16>,
    pub(crate) user: Option<String>,
    pub(crate) auth: SshAuthDto,
    /// `[user@]host[:port]` entries, first hop first.
    pub(crate) jump_hosts: Vec<String>,
    pub(crate) password: SecretInput,
    pub(crate) passphrase: SecretInput,
}

/// Saved TLS certificate files and whether verification is skipped.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TlsFilesDto {
    ca_file: Option<String>,
    /// A bundled cloud provider CA instead of a CA file.
    cloud_ca: Option<CloudCaDto>,
    client_cert: Option<String>,
    client_key: Option<String>,
    has_key_passphrase: bool,
    /// The server certificate of a remote host is not authenticated: show
    /// the skip-verification warning.
    unverified_remote: bool,
}

/// TLS certificate files from the form.
#[derive(Clone, Debug, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TlsFilesInput {
    pub(crate) ca_file: Option<String>,
    /// Absent in forms written before cloud CAs: none.
    #[serde(default)]
    pub(crate) cloud_ca: Option<CloudCaDto>,
    pub(crate) client_cert: Option<String>,
    pub(crate) client_key: Option<String>,
    pub(crate) key_passphrase: SecretInput,
}

pub(crate) fn tls_files_dto(profile: &ConnectionProfile) -> TlsFilesDto {
    let tls = &profile.tls;
    TlsFilesDto {
        ca_file: tls.ca_file.as_deref().map(display),
        cloud_ca: tls.cloud_ca.map(Into::into),
        client_cert: tls.client_cert.as_deref().map(display),
        client_key: tls.client_key.as_deref().map(display),
        has_key_passphrase: profile.secrets.tls_key_passphrase.is_some(),
        unverified_remote: tls.unverified_remote(&profile.params.host),
    }
}

pub(crate) fn ssh_dto(profile: &ConnectionProfile) -> Option<SshDto> {
    let ssh = profile.ssh.as_ref()?;
    Some(SshDto {
        host: ssh.host.clone(),
        port: ssh.port,
        user: ssh.user.clone(),
        auth: match &ssh.auth {
            SshAuthMethod::Password => SshAuthDto::Password,
            SshAuthMethod::PrivateKey { key_file } => SshAuthDto::PrivateKey {
                key_file: display(key_file),
            },
            SshAuthMethod::Agent => SshAuthDto::Agent,
            SshAuthMethod::KeyboardInteractive => SshAuthDto::KeyboardInteractive,
        },
        jump_hosts: ssh.jump_hosts.clone(),
        has_password: profile.secrets.get(SecretSlot::SshPassword).is_some(),
        has_passphrase: profile.secrets.get(SecretSlot::SshPassphrase).is_some(),
    })
}

/// Copy the form's TLS files, SSH settings and proxy onto `profile`.
pub(crate) fn apply_route(
    tls: &TlsFilesInput,
    ssh: Option<&SshInput>,
    proxy: Option<&ProxyInput>,
    profile: &mut ConnectionProfile,
) {
    profile.proxy = proxy.map(proxy_settings);
    profile.tls.ca_file = path(tls.ca_file.as_deref());
    profile.tls.cloud_ca = tls.cloud_ca.map(Into::into);
    profile.tls.client_cert = path(tls.client_cert.as_deref());
    profile.tls.client_key = path(tls.client_key.as_deref());
    profile.ssh = ssh.map(|input| SshSettings {
        host: input.host.trim().to_owned(),
        port: input.port,
        user: non_blank(input.user.as_deref()),
        auth: match &input.auth {
            SshAuthDto::Password => SshAuthMethod::Password,
            SshAuthDto::PrivateKey { key_file } => SshAuthMethod::PrivateKey {
                key_file: PathBuf::from(key_file.trim()),
            },
            SshAuthDto::Agent => SshAuthMethod::Agent,
            SshAuthDto::KeyboardInteractive => SshAuthMethod::KeyboardInteractive,
        },
        jump_hosts: input
            .jump_hosts
            .iter()
            .filter_map(|jump| non_blank(Some(jump)))
            .collect(),
    });
}

fn display(path: &Path) -> String {
    path.display().to_string()
}

fn path(text: Option<&str>) -> Option<PathBuf> {
    non_blank(text).map(PathBuf::from)
}

fn non_blank(text: Option<&str>) -> Option<String> {
    text.map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use openrows_store::profile::{EnvKind, EnvTag, SecretRef, ServerParams};

    pub(crate) fn ssh_input(auth: SshAuthDto) -> SshInput {
        SshInput {
            host: " bastion ".to_owned(),
            port: Some(2222),
            user: Some(" ".to_owned()),
            auth,
            jump_hosts: vec!["ops@jump:22".to_owned(), "  ".to_owned()],
            password: SecretInput::Keep,
            passphrase: SecretInput::Keep,
        }
    }

    #[test]
    fn route_fields_are_trimmed_and_blank_entries_dropped() {
        let mut profile = ConnectionProfile::new_postgres(
            "p",
            ServerParams::default(),
            EnvTag::new(EnvKind::Dev),
        );
        let tls = TlsFilesInput {
            ca_file: Some(" /certs/ca.pem ".to_owned()),
            client_cert: Some(String::new()),
            ..TlsFilesInput::default()
        };
        let key = SshAuthDto::PrivateKey {
            key_file: "~/.ssh/id_ed25519".to_owned(),
        };
        apply_route(&tls, Some(&ssh_input(key)), None, &mut profile);
        assert_eq!(profile.tls.ca_file, Some(PathBuf::from("/certs/ca.pem")));
        assert_eq!(profile.tls.client_cert, None);
        let ssh = profile
            .ssh
            .clone()
            .unwrap_or_else(|| unreachable!("SSH set"));
        assert_eq!((ssh.host.as_str(), ssh.user), ("bastion", None));
        assert_eq!(ssh.jump_hosts, ["ops@jump:22"]);
        apply_route(&TlsFilesInput::default(), None, None, &mut profile);
        assert_eq!(profile.ssh, None);
    }

    #[test]
    fn dtos_say_whether_secrets_exist_but_never_carry_them() -> Result<(), serde_json::Error> {
        let mut profile = ConnectionProfile::new_postgres(
            "p",
            ServerParams::default(),
            EnvTag::new(EnvKind::Dev),
        );
        apply_route(
            &TlsFilesInput::default(),
            Some(&ssh_input(SshAuthDto::Password)),
            None,
            &mut profile,
        );
        profile.secrets.ssh_password = Some(SecretRef::Keychain);
        let json = serde_json::to_value(ssh_dto(&profile))?;
        assert_eq!(json["hasPassword"], true);
        assert_eq!(json["auth"]["method"], "password");
        assert!(json.get("password").is_none());
        let tls = serde_json::to_value(tls_files_dto(&profile))?;
        assert_eq!(tls["unverifiedRemote"], false, "localhost is not remote");
        profile.params.host = "db.example.com".to_owned();
        let tls = serde_json::to_value(tls_files_dto(&profile))?;
        assert_eq!(tls["unverifiedRemote"], true, "prefer never verifies");
        Ok(())
    }

    #[test]
    fn debug_output_never_contains_secrets() {
        let mut input = ssh_input(SshAuthDto::Password);
        input.password = SecretInput::Set {
            value: "ssh-pw-synthetic".to_owned(),
        };
        assert!(!format!("{input:?}").contains("ssh-pw-synthetic"));
    }

    #[test]
    fn secret_inputs_map_to_changes() {
        assert!(matches!(SecretInput::Keep.change(), PasswordChange::Keep));
        assert!(matches!(SecretInput::Clear.change(), PasswordChange::Clear));
        let set = SecretInput::Set {
            value: "otp-synthetic".to_owned(),
        };
        assert!(
            matches!(set.change(), PasswordChange::Set(value) if value.expose() == "otp-synthetic")
        );
    }
}
