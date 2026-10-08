// SPDX-License-Identifier: Apache-2.0
//! The route step of every connect (D-10, D-11): keychain secrets, TLS files
//! and, for profiles with SSH, the profile's shared tunnel. The result is a
//! driver [`Target`] that says where to connect (direct, or a stream through
//! the tunnel) and with which TLS parameters. Engine-agnostic: the
//! profile's driver is resolved in [`super::target`].
use std::sync::Arc;

use openrows_net::TunnelDialer;
use openrows_store::profile::{ConnectionProfile, ProfileService, SecretSlot};

use super::Connections;
use super::error::IpcError;
use super::profiles::profile_id;
use super::target::{self, DriverSecrets, Target};
use super::tunnels::{SshSecrets, route_spec};

/// Driver secrets of a saved profile; the keychain is read only for secrets
/// the profile references.
///
/// # Errors
/// `Secret` when the keychain cannot be read.
pub(crate) fn driver_secrets(
    service: &ProfileService,
    profile: &ConnectionProfile,
) -> Result<DriverSecrets, IpcError> {
    Ok(DriverSecrets {
        password: service.secret(profile, SecretSlot::Password)?,
        tls_key_passphrase: service.secret(profile, SecretSlot::TlsKeyPassphrase)?,
    })
}

/// SSH secrets of a saved profile.
///
/// # Errors
/// `Secret` when the keychain cannot be read.
pub(crate) fn ssh_secrets(
    service: &ProfileService,
    profile: &ConnectionProfile,
) -> Result<SshSecrets, IpcError> {
    Ok(SshSecrets {
        password: service.secret(profile, SecretSlot::SshPassword)?,
        passphrase: service.secret(profile, SecretSlot::SshPassphrase)?,
    })
}

/// Where and how to reach saved connection `id`: its driver target, through
/// the profile's shared SSH tunnel (opened on first use, with prompts) when
/// it has one.
///
/// # Errors
/// Profile, keychain, SSH (typed: `ssh`, `hostKey`, `cancelled`) or
/// validation failures.
pub(crate) async fn profile_target(state: &Connections, id: &str) -> Result<Target, IpcError> {
    let profile = state.profiles.get(profile_id(id)?)?;
    let secrets = driver_secrets(&state.profiles, &profile)?;
    let target = target::target(&profile, &secrets)?;
    let Some(ssh) = &profile.ssh else {
        return Ok(target);
    };
    let open = async {
        let spec = route_spec(ssh, ssh_secrets(&state.profiles, &profile)?)?;
        state.opener.connect(&spec).await
    };
    let tunnel = state.tunnels.get_or_open(id, Box::pin(open)).await?;
    let dialer = TunnelDialer::new(tunnel, &profile.params.host, profile.params.port);
    Ok(target.through(Arc::new(dialer)))
}
