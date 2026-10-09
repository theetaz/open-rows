// SPDX-License-Identifier: Apache-2.0
//! Literal credential invocations and typed failures; trust is stored separately.
use super::ProfileError;
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// A program invoked directly, never interpreted by a shell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialCommand {
    /// Absolute executable path, shown verbatim in the trust prompt.
    pub program: PathBuf,
    /// Literal arguments; these must not contain credentials.
    #[serde(default)]
    pub args: Vec<String>,
    /// Maximum runtime in seconds, inclusive range 1–120.
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u16,
}
fn default_timeout() -> u16 {
    10
}
impl CredentialCommand {
    /// Validate the executable path and bounded deadline.
    /// # Errors
    /// An invalid path, argument or deadline.
    pub fn validate(&self) -> Result<(), ProfileError> {
        if !self.program.is_absolute() || self.program.as_os_str().is_empty() {
            return Err(ProfileError::Invalid {
                field: "credentialCommand.program",
                reason: "must be an absolute program path",
            });
        }
        if !(1..=120).contains(&self.timeout_seconds) {
            return Err(ProfileError::Invalid {
                field: "credentialCommand.timeoutSeconds",
                reason: "must be 1–120 seconds",
            });
        }
        if self.args.iter().any(|arg| arg.contains('\0')) {
            return Err(ProfileError::Invalid {
                field: "credentialCommand.args",
                reason: "must not contain NUL",
            });
        }
        Ok(())
    }
}

/// Failures safe to send to the UI, without raw child output.
#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    /// This profile and command hash have not been trusted locally.
    #[error("Credential command requires Trust and run on this machine")]
    Untrusted,
    /// The child exceeded its deadline.
    #[error("Credential command timed out")]
    Timeout,
    /// The user cancelled this attempt.
    #[error("Credential command cancelled")]
    Cancelled,
    /// Starting, reading, or waiting on the process failed.
    #[error("Credential command process failed")]
    Process(#[source] std::io::Error),
    /// Nonzero exit or missing first-line password; diagnostics are scrubbed.
    #[error("Credential command failed: {0}")]
    Failed(String),
    /// Invalid configuration or a trust-store write failed.
    #[error(transparent)]
    Profile(#[from] ProfileError),
}

/// Cooperative cancellation shared with the connect progress UI.
#[derive(Clone, Debug, Default)]
pub struct CommandCancellation(Arc<AtomicBool>);
impl CommandCancellation {
    /// Request termination of the child and its process group.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    /// Whether cancellation was requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
