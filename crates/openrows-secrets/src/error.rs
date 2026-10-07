// SPDX-License-Identifier: Apache-2.0
//! Typed failures of secret stores. Messages name the account, never a value.

/// Every way a secret store operation can fail.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SecretError {
    /// The OS credential store rejected or failed the operation.
    #[error("keychain operation on {account} failed")]
    Platform {
        /// Keychain account the operation targeted.
        account: String,
        /// Backend error (contains no secret material).
        #[source]
        source: keyring::Error,
    },
    /// A thread panicked while holding the in-memory store lock.
    #[error("in-memory secret store lock poisoned")]
    Poisoned,
}
