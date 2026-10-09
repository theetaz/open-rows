// SPDX-License-Identifier: Apache-2.0
//! Typed, scrubbed helper failures.
/// A failure to start, maintain or use a cloud route.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum HelperError {
    /// Invalid structured fields.
    #[error("{0}")]
    Invalid(String),
    /// No executable was found.
    #[error("{tool} was not found. Install it or set its path in Settings ▸ Tools.")]
    NotFound {
        /// Executable name.
        tool: String,
    },
    /// Start, timeout, exit or shutdown failure with bounded scrubbed stderr.
    #[error("{tool}: {message}\n{stderr}\n{hint}")]
    Network {
        /// Executable name.
        tool: String,
        /// User-readable reason.
        message: String,
        /// Last 200 stderr lines, redacted.
        stderr: String,
        /// Kind-specific advice.
        hint: String,
    },
}
