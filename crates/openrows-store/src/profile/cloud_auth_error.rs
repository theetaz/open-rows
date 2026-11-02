// SPDX-License-Identifier: Apache-2.0
//! Typed cloud token failures that say what to change; none carries a token.
use super::ProfileError;
use openrows_net::helper::scrub;

/// Longest CLI diagnostic kept in an error message.
const DETAIL_LIMIT: usize = 400;

/// Why no cloud token reached the database. Safe to send to the UI.
#[derive(Debug, thiserror::Error)]
pub enum CloudAuthError {
    /// The CLI is not on this machine.
    #[error(
        "{tool} is not installed. Install it or set its path in Settings ▸ Tools, then connect again."
    )]
    NotInstalled {
        /// Executable name.
        tool: &'static str,
    },
    /// The CLI has no usable session, or it expired.
    #[error(
        "You are signed out of {tool} or your session expired. Run {login} in a terminal, then connect again."
    )]
    SignedOut {
        /// Executable name.
        tool: &'static str,
        /// The command that signs in again.
        login: &'static str,
    },
    /// The configured profile, region, tenant or account is not known.
    #[error(
        "{tool} does not accept this connection's cloud settings; check its profile, region, tenant or account. {detail}"
    )]
    WrongTarget {
        /// Executable name.
        tool: &'static str,
        /// Scrubbed explanation.
        detail: String,
    },
    /// The signed-in identity may not get this token.
    #[error("{tool} refused: your identity lacks permission for this database token. {detail}")]
    PermissionDenied {
        /// Executable name.
        tool: &'static str,
        /// Scrubbed explanation.
        detail: String,
    },
    /// The route could expose the token.
    #[error("Refusing to send a cloud token because {reason}. {fix}")]
    InsecureRoute {
        /// What is wrong with the route.
        reason: &'static str,
        /// What to change.
        fix: &'static str,
    },
    /// This command and destination have not been approved on this machine.
    #[error(
        "This cloud token command and database host need your approval. Connect again to review them."
    )]
    Untrusted,
    /// The CLI exceeded its deadline.
    #[error("{tool} did not return a token in time. Check your network and try again.")]
    Timeout {
        /// Executable name.
        tool: &'static str,
    },
    /// The user cancelled this attempt.
    #[error("Cloud token request cancelled")]
    Cancelled,
    /// Anything else the CLI reported.
    #[error("{tool} did not return a token. {detail}")]
    Failed {
        /// Executable name.
        tool: &'static str,
        /// Scrubbed explanation.
        detail: String,
    },
    /// Invalid settings, or the local trust file failed.
    #[error(transparent)]
    Profile(#[from] ProfileError),
}

/// Lowercase phrases of the three CLIs' own messages, most specific kind first.
const SIGNED_OUT: [&str; 21] = [
    "unable to locate credentials",
    "expiredtoken",
    "expired token",
    "token has expired",
    "token is expired",
    "session has expired",
    "sso session",
    "error loading sso token",
    "sso login",
    "aws login",
    "az login",
    "refresh token has expired",
    "interaction_required",
    "aadsts700082",
    "aadsts50173",
    "aadsts70043",
    "gcloud auth login",
    "do not currently have an active account",
    "problem refreshing your current auth tokens",
    "reauthentication",
    "invalid_grant",
];
const WRONG_TARGET: [&str; 8] = [
    "could not be found",
    "you must specify a region",
    "provided region_name",
    "invalid endpoint",
    "aadsts90002",
    "aadsts900023",
    "no subscription found",
    "tenant",
];
const DENIED: [&str; 10] = [
    "accessdenied",
    "access denied",
    "not authorized",
    "unauthorizedoperation",
    "permission denied",
    "permission_denied",
    "does not have permission",
    "forbidden",
    "aadsts65001",
    "aadsts53003",
];

/// Turn a CLI's scrubbed stderr into a typed, actionable error.
pub(super) fn classify(
    tool: &'static str,
    login: &'static str,
    diagnostics: &str,
) -> CloudAuthError {
    let detail = safe_detail(diagnostics);
    let lower = detail.to_ascii_lowercase();
    let mentions = |phrases: &[&str]| phrases.iter().any(|phrase| lower.contains(phrase));
    if mentions(&SIGNED_OUT) {
        CloudAuthError::SignedOut { tool, login }
    } else if mentions(&DENIED) {
        CloudAuthError::PermissionDenied { tool, detail }
    } else if mentions(&WRONG_TARGET) {
        CloudAuthError::WrongTarget { tool, detail }
    } else {
        CloudAuthError::Failed { tool, detail }
    }
}

/// Drop credential-like lines and anything shaped like a JWT (`eyJ…`), which
/// carries no `token=` marker for the shared scrubber to find.
fn safe_detail(diagnostics: &str) -> String {
    let lines: Vec<String> = diagnostics
        .lines()
        .map(|line| {
            if line.contains("eyJ") || line.to_ascii_lowercase().contains("accesstoken") {
                "[credential-bearing stderr redacted]".to_owned()
            } else {
                scrub(line)
            }
        })
        .collect();
    lines.join("\n").chars().take(DETAIL_LIMIT).collect()
}
