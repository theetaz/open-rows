// SPDX-License-Identifier: Apache-2.0
//! Custom helpers reuse the credential trust store with a distinct purpose.
use super::{
    CommandPurpose, CommandTrust, ConnectionProfile, CredentialCommand, CredentialError, HelperHop,
};
use openrows_net::helper::HelperSpec;

/// The unexpanded invocation shown in the hop trust confirmation.
#[must_use]
pub fn hop_command(profile: &ConnectionProfile) -> Option<CredentialCommand> {
    let HelperHop::Custom { program, args, .. } = profile.helper.as_ref()? else {
        return None;
    };
    Some(CredentialCommand {
        program: program.into(),
        args: args.clone(),
        timeout_seconds: 10,
    })
}

/// Resolve a custom helper only after checking its template and local hop approval.
/// # Errors
/// Invalid profile, untrusted template or unreadable local trust.
pub fn trusted_hop_spec(
    profile: &ConnectionProfile,
    trust: &CommandTrust,
) -> Result<Option<HelperSpec>, CredentialError> {
    let Some(command) = hop_command(profile) else {
        return Ok(None);
    };
    profile.validate()?;
    trust.require_for(profile.id, CommandPurpose::Hop, &command)?;
    let hop = profile.helper.clone().ok_or(CredentialError::Untrusted)?;
    Ok(Some(
        HelperSpec::new(hop, command.program, "not probed (custom command)".into())
            .with_destination(profile.params.host.clone(), profile.params.port),
    ))
}

/// Start or share a custom helper only after the local hop trust gate.
/// # Errors
/// Untrusted invocation, invalid configuration or helper launch/readiness failure.
pub async fn acquire_trusted_hop(
    pool: &openrows_net::helper::Helpers,
    profile: &ConnectionProfile,
    trust: &CommandTrust,
) -> Result<std::sync::Arc<openrows_net::helper::Helper>, CredentialError> {
    let spec = trusted_hop_spec(profile, trust)?.ok_or(CredentialError::Untrusted)?;
    pool.acquire(&profile.id.to_string(), spec)
        .await
        .map_err(CredentialError::Helper)
}
