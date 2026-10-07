// SPDX-License-Identifier: Apache-2.0
//! Credential storage for `OpenRows` (docs/12 §3, FR-CONN-03).
//!
//! Secrets live only in a [`SecretStore`]: the OS keychain in the app
//! ([`KeychainSecretStore`]) or memory in tests ([`MemorySecretStore`]).
//! Profiles on disk hold a [`SecretKey`] reference, never the value.

#![forbid(unsafe_code)]

mod error;
mod key;
mod keychain;
mod memory;
mod secret;

pub use error::SecretError;
pub use key::{SERVICE, SecretKey};
pub use keychain::KeychainSecretStore;
pub use memory::MemorySecretStore;
pub use secret::SecretString;

/// A place secrets can be written to, read from and removed from.
///
/// Implementations must never log or `Debug`-print secret values.
pub trait SecretStore: Send + Sync {
    /// Store `value` under `key`, replacing any previous value.
    ///
    /// # Errors
    /// Returns [`SecretError`] when the backend rejects the write.
    fn set(&self, key: &SecretKey, value: &SecretString) -> Result<(), SecretError>;

    /// Read the value under `key`; `None` when nothing is stored.
    ///
    /// # Errors
    /// Returns [`SecretError`] when the backend cannot be read.
    fn get(&self, key: &SecretKey) -> Result<Option<SecretString>, SecretError>;

    /// Remove the value under `key`; removing a missing key succeeds.
    ///
    /// # Errors
    /// Returns [`SecretError`] when the backend rejects the delete.
    fn delete(&self, key: &SecretKey) -> Result<(), SecretError>;
}
