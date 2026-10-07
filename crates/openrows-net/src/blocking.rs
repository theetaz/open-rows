// SPDX-License-Identifier: Apache-2.0
//! Blocking work (file I/O, key decryption) kept off the async runtime.

use crate::SshError;

/// Run `work` on tokio's blocking pool.
pub(crate) async fn off_runtime<T, F>(work: F) -> Result<T, SshError>
where
    F: FnOnce() -> Result<T, SshError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| SshError::BackgroundTask)?
}
