// SPDX-License-Identifier: Apache-2.0
//! Bounded host failures never include SQL, credentials or native diagnostics.

/// Why an isolated connection must be discarded without reconnect or replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum HostFailure {
    /// The helper could not start, or its crash circuit is open.
    #[error("driver host could not start; reconnect explicitly")]
    Spawn,
    /// The app and helper do not have the same build or wire contract.
    #[error("driver host is incompatible with this app")]
    Incompatible,
    /// The child exited or its control connection ended unexpectedly.
    #[error("driver host exited; the operation outcome may be uncertain")]
    Exited,
    /// The host or aggregate memory reservation exceeded policy.
    #[error("driver host memory limit reached")]
    MemoryLimit,
    /// The heartbeat probe or shutdown/cancel grace expired.
    #[error("driver host stopped responding")]
    Unresponsive,
    /// A framing, authentication, Arrow or lifecycle invariant failed.
    #[error("driver host protocol failed")]
    Protocol,
}
