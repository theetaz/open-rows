// SPDX-License-Identifier: Apache-2.0
//! Non-secret credential command configuration crossing IPC.
use openrows_store::profile::{CloudAuth, CredentialCommand};
use serde::{Deserialize, Serialize};
use specta::Type;
/// Exact invocation shown to the user before locally trusting it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CredentialCommandDto {
    program: String,
    args: Vec<String>,
    timeout_seconds: u16,
}
/// Non-secret cloud sign-in settings; the token never crosses IPC.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "provider", rename_all = "camelCase")]
pub(crate) enum CloudAuthDto {
    /// RDS and Aurora IAM through the AWS CLI.
    Aws {
        region: String,
        profile: Option<String>,
    },
    /// Entra ID tokens through the Azure CLI.
    Azure { tenant: Option<String> },
    /// Cloud SQL IAM through gcloud.
    Google { account: Option<String> },
}
impl From<&CloudAuth> for CloudAuthDto {
    fn from(auth: &CloudAuth) -> Self {
        match auth.clone() {
            CloudAuth::Aws { region, profile } => Self::Aws { region, profile },
            CloudAuth::Azure { tenant } => Self::Azure { tenant },
            CloudAuth::Google { account } => Self::Google { account },
        }
    }
}
impl From<&CloudAuthDto> for CloudAuth {
    fn from(auth: &CloudAuthDto) -> Self {
        match auth.clone() {
            CloudAuthDto::Aws { region, profile } => Self::Aws { region, profile },
            CloudAuthDto::Azure { tenant } => Self::Azure { tenant },
            CloudAuthDto::Google { account } => Self::Google { account },
        }
    }
}
impl From<&CredentialCommand> for CredentialCommandDto {
    fn from(command: &CredentialCommand) -> Self {
        Self {
            program: command.program.display().to_string(),
            args: command.args.clone(),
            timeout_seconds: command.timeout_seconds,
        }
    }
}
impl From<&CredentialCommandDto> for CredentialCommand {
    fn from(command: &CredentialCommandDto) -> Self {
        Self {
            program: command.program.clone().into(),
            args: command.args.clone(),
            timeout_seconds: command.timeout_seconds,
        }
    }
}
