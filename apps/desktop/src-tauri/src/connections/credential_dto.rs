// SPDX-License-Identifier: Apache-2.0
//! Non-secret credential command configuration crossing IPC.
use openrows_store::profile::CredentialCommand;
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
