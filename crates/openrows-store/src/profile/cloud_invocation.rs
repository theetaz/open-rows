// SPDX-License-Identifier: Apache-2.0
//! Running a signed-in cloud CLI for one connect, after the settings, the
//! route and the user's approval of this command and destination (docs/24 §19).
use super::{
    CloudAuth, CommandCancellation, CommandPurpose, CommandTrust, ConnectionProfile,
    CredentialCommand, CredentialError, ProfileError, ProfileId, credential_run,
};
use crate::profile::cloud_auth_error::{CloudAuthError, classify};
use openrows_net::helper::tools;
use openrows_secrets::SecretString;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

/// The CLIs may refresh a session over the network before answering.
const TOKEN_TIMEOUT_SECONDS: u16 = 30;

/// One checked way to get a token for one profile and destination.
#[derive(Debug, Clone)]
pub struct CloudInvocation {
    tool: &'static str,
    login: &'static str,
    command: CredentialCommand,
    destination: String,
    search_path: Option<OsString>,
}

impl CloudInvocation {
    /// Check the settings and the route, then find the tool. Nothing runs
    /// here, so an insecure route never produces a token.
    ///
    /// # Errors
    /// Invalid settings, an insecure route, a mismatched region or a missing tool.
    pub fn new(
        profile: &ConnectionProfile,
        override_path: Option<&Path>,
        dirs: &[PathBuf],
    ) -> Result<Self, CloudAuthError> {
        profile.validate_cloud_auth()?;
        let auth = profile.cloud_auth.as_ref().ok_or(ProfileError::Invalid {
            field: "cloudAuth",
            reason: "choose a cloud provider",
        })?;
        auth.require_secure_route(profile)?;
        auth.require_matching_region(&profile.params.host)?;
        let tool = auth.tool();
        let program = tools::locate(tool, override_path, dirs)
            .map_err(|_| CloudAuthError::NotInstalled { tool })?;
        Ok(Self {
            tool,
            login: login_command(auth),
            command: CredentialCommand {
                program,
                args: auth.arguments(profile)?,
                timeout_seconds: TOKEN_TIMEOUT_SECONDS,
            },
            destination: format!("{}:{}", profile.params.host, profile.params.port),
            search_path: std::env::join_paths(dirs).ok(),
        })
    }

    /// The exact program and arguments, shown before approval.
    #[must_use]
    pub fn command(&self) -> &CredentialCommand {
        &self.command
    }

    /// `host:port` that receives the token.
    #[must_use]
    pub fn destination(&self) -> &str {
        &self.destination
    }

    /// Whether the user approved this command and destination on this machine.
    ///
    /// # Errors
    /// The local trust file cannot be read.
    pub fn is_trusted(&self, trust: &CommandTrust, id: ProfileId) -> Result<bool, ProfileError> {
        trust.is_trusted_for(id, CommandPurpose::CloudToken, &self.approval())
    }

    /// Record the user's explicit approval of this command and destination.
    ///
    /// # Errors
    /// The local trust file cannot be written.
    pub fn trust(&self, trust: &CommandTrust, id: ProfileId) -> Result<(), ProfileError> {
        trust.trust_for(id, CommandPurpose::CloudToken, &self.approval())
    }

    /// Get a token for one connect attempt. Azure and Google tokens are bearer
    /// tokens valid for other databases too, so the approval covers the host:
    /// an edited or imported destination cannot receive one unapproved.
    ///
    /// # Errors
    /// Untrusted, timed out, cancelled, signed out, wrong settings, denied or
    /// a failed CLI, all without child output beyond scrubbed diagnostics.
    pub fn run(
        &self,
        trust: &CommandTrust,
        id: ProfileId,
        cancel: &CommandCancellation,
    ) -> Result<SecretString, CloudAuthError> {
        if !self.is_trusted(trust, id)? {
            return Err(CloudAuthError::Untrusted);
        }
        let token =
            credential_run::run_approved(&self.command, self.search_path.as_deref(), cancel)
                .map_err(|error| self.failure(error))?;
        if token.expose().chars().any(char::is_control) {
            return Err(CloudAuthError::Failed {
                tool: self.tool,
                detail: "Its output is not a single-line token.".into(),
            });
        }
        Ok(token)
    }

    /// The approved identity: the program, its arguments and the destination.
    fn approval(&self) -> CredentialCommand {
        let mut approval = self.command.clone();
        approval
            .args
            .push(format!("token-destination={}", self.destination));
        approval
    }

    fn failure(&self, error: CredentialError) -> CloudAuthError {
        match error {
            CredentialError::Timeout => CloudAuthError::Timeout { tool: self.tool },
            CredentialError::Cancelled => CloudAuthError::Cancelled,
            CredentialError::Process(source) if source.kind() == std::io::ErrorKind::NotFound => {
                CloudAuthError::NotInstalled { tool: self.tool }
            }
            CredentialError::Failed(diagnostics) => classify(self.tool, self.login, &diagnostics),
            CredentialError::Profile(source) => CloudAuthError::Profile(source),
            CredentialError::Untrusted => CloudAuthError::Untrusted,
            CredentialError::Process(_) | CredentialError::Helper(_) => CloudAuthError::Failed {
                tool: self.tool,
                detail: "The program could not be run.".into(),
            },
        }
    }
}

fn login_command(auth: &CloudAuth) -> &'static str {
    match auth {
        CloudAuth::Aws { .. } => "aws sso login or aws configure",
        CloudAuth::Azure { .. } => "az login",
        CloudAuth::Google { .. } => "gcloud auth login",
    }
}
