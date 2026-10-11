// SPDX-License-Identifier: Apache-2.0
//! Fixed error messages never include URLs, HTTP bodies, headers or credentials.
use serde::{Deserialize, Serialize};

/// Safe provider failure categories shared by transports and the settings UI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AiError {
    /// Required local audit persistence failed; no outbound request is sent.
    #[error("AI audit store is unavailable")]
    Audit,
    /// Credentials were missing or rejected.
    #[error("Provider authentication failed")]
    Auth,
    /// The provider requests a delay before retrying; no automatic retry occurs.
    #[error("Provider rate limit reached")]
    #[serde(rename_all = "camelCase")]
    RateLimit {
        /// Reported Retry-After delay in seconds, if present.
        retry_after: Option<u32>,
    },
    /// Billing or credit quota is exhausted.
    #[error("Provider quota exhausted")]
    Quota,
    /// Transport failed without exposing the underlying message.
    #[error("Provider network request failed")]
    Network,
    /// The selected operation or endpoint is unavailable.
    #[error("Provider operation is unsupported")]
    Unsupported,
    /// The wire response was malformed, oversized or incomplete.
    #[error("Provider returned an invalid response")]
    InvalidResponse,
    /// The endpoint/configuration violates the settings contract.
    #[error("Invalid provider configuration or endpoint")]
    InvalidConfig,
    /// Global AI permission is off.
    #[error("AI features are off")]
    Disabled,
    /// Explicit cancellation interrupted the operation.
    #[error("Provider request cancelled")]
    Cancelled,
    /// The bounded total request deadline elapsed.
    #[error("Provider request timed out")]
    Timeout,
}
