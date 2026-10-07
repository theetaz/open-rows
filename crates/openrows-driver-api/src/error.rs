// SPDX-License-Identifier: Apache-2.0
use arrow_schema::ArrowError;

/// Typed driver failures; the engine maps these onto `OdbError` (docs/04 §7).
#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    /// Socket or transport failure; the session is no longer usable.
    #[error("network error: {0}")]
    Network(#[from] std::io::Error),
    /// The server rejected the credentials or required an unsupported method.
    #[error("authentication failed: {0}")]
    Auth(String),
    /// The server sent a message the driver did not expect at this point.
    #[error("protocol violation: {0}")]
    Protocol(String),
    /// The server reported an error for the statement; the session stays usable.
    #[error("{message} (SQLSTATE {code})")]
    Server {
        /// Five-character SQLSTATE.
        code: String,
        /// Human-readable server message.
        message: String,
    },
    /// A column value could not be decoded into its Arrow representation.
    #[error("column {column}: {reason}")]
    Decode {
        /// Zero-based result column index.
        column: usize,
        /// Why the value was rejected.
        reason: String,
    },
    /// The request uses a feature this driver does not implement.
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// Arrow rejected a schema or batch.
    #[error(transparent)]
    Arrow(#[from] ArrowError),
}
