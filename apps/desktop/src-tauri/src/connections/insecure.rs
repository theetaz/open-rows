// SPDX-License-Identifier: Apache-2.0
//! Why a connection is flagged as insecure (docs/12 §4.1, M1-06): the
//! connection list shows the profile's reasons, the window chrome adds the
//! live tunnel's (a host key accepted for this connection only).
use openrows_store::profile::{ConnectionProfile, Insecurity};
use serde::Serialize;
use specta::Type;

/// One reason for the insecure badge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum InsecureReasonDto {
    /// TLS is off towards a remote host.
    TlsDisabled,
    /// The remote server's certificate is not verified.
    TlsUnverified,
    /// An unknown SSH host key was accepted without saving it.
    HostKeyNotSaved,
}

/// Reasons that follow from the saved settings alone.
pub(crate) fn profile_insecurity(profile: &ConnectionProfile) -> Vec<InsecureReasonDto> {
    if profile.driver.is_file() {
        return Vec::new();
    }
    let reason = profile
        .tls
        .insecurity(&profile.params.host)
        .map(|insecurity| match insecurity {
            Insecurity::TlsDisabled => InsecureReasonDto::TlsDisabled,
            Insecurity::TlsUnverified => InsecureReasonDto::TlsUnverified,
        });
    reason.into_iter().collect()
}

/// The profile's reasons plus the live route's.
pub(crate) fn connection_insecurity(
    profile: &ConnectionProfile,
    unsaved_host_keys: bool,
) -> Vec<InsecureReasonDto> {
    let mut reasons = profile_insecurity(profile);
    if unsaved_host_keys {
        reasons.push(InsecureReasonDto::HostKeyNotSaved);
    }
    reasons
}

#[cfg(test)]
mod tests {
    use openrows_store::profile::{
        EnvKind, EnvTag, FileParams, ServerParams, TlsMode, TlsSettings,
    };

    use super::*;

    fn server(host: &str, mode: TlsMode) -> ConnectionProfile {
        let params = ServerParams {
            host: host.to_owned(),
            ..ServerParams::default()
        };
        let mut profile = ConnectionProfile::new_postgres("p", params, EnvTag::new(EnvKind::Dev));
        profile.tls = TlsSettings::new(mode);
        profile
    }

    #[test]
    fn remote_hosts_without_verified_tls_are_flagged() {
        let off = server("db.example.com", TlsMode::Disable);
        assert_eq!(profile_insecurity(&off), [InsecureReasonDto::TlsDisabled]);
        let unverified = server("db.example.com", TlsMode::Require);
        assert_eq!(
            profile_insecurity(&unverified),
            [InsecureReasonDto::TlsUnverified]
        );
        assert!(profile_insecurity(&server("db.example.com", TlsMode::VerifyFull)).is_empty());
        assert!(profile_insecurity(&server("localhost", TlsMode::Disable)).is_empty());
    }

    #[test]
    fn host_keys_accepted_once_are_added_and_files_never_flagged() {
        let verified = server("db.example.com", TlsMode::VerifyFull);
        assert_eq!(
            connection_insecurity(&verified, true),
            [InsecureReasonDto::HostKeyNotSaved]
        );
        let file = ConnectionProfile::new_file(
            "f",
            FileParams {
                path: "/tmp/db.sqlite".into(),
                read_only: false,
                create: false,
            },
            EnvTag::new(EnvKind::Local),
        );
        assert!(profile_insecurity(&file).is_empty());
    }
}
