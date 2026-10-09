// SPDX-License-Identifier: Apache-2.0
use arrow_schema::ArrowError;

/// Typed driver failures; the engine maps these onto `OdbError` (docs/04 §7).
#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    /// Socket or transport failure; the session is no longer usable.
    #[error("network error: {0}")]
    Network(#[from] std::io::Error),
    /// TLS could not be set up: certificate files, verification or handshake.
    /// The message says what to change; the session is not usable.
    #[error("TLS error: {0}")]
    Tls(String),
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

impl DriverError {
    /// True when the transport is gone (closed, reset, timed out): the
    /// connection cannot run another statement and must be replaced.
    #[must_use]
    pub fn is_connection_lost(&self) -> bool {
        matches!(self, Self::Network(_))
    }

    /// Explain a lost connection during a transaction that had not sent
    /// `COMMIT`: the server rolls an open transaction back when its
    /// connection ends, so nothing was applied. Other errors are unchanged.
    #[must_use]
    pub fn lost_before_commit(self) -> Self {
        match self {
            Self::Network(error) => Self::Network(std::io::Error::new(
                error.kind(),
                format!(
                    "the connection was lost before COMMIT, so the server rolled the \
                     transaction back and nothing was changed ({error})"
                ),
            )),
            other => other,
        }
    }

    /// Name the failing startup command (`index` from 0) in a server error.
    #[must_use]
    pub fn in_startup_command(self, index: usize) -> Self {
        match self {
            Self::Server { code, message } => Self::Server {
                code,
                message: format!("startup command {}: {message}", index + 1),
            },
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_transport_failures_lose_the_connection() {
        let lost = DriverError::Network(std::io::ErrorKind::UnexpectedEof.into());
        assert!(lost.is_connection_lost());
        let server = DriverError::Server {
            code: "40001".to_owned(),
            message: "conflict".to_owned(),
        };
        assert!(!server.is_connection_lost());
        assert!(!DriverError::Protocol("bad".to_owned()).is_connection_lost());
    }

    #[test]
    fn a_loss_before_commit_says_the_transaction_was_rolled_back() {
        let lost = DriverError::Network(std::io::ErrorKind::BrokenPipe.into()).lost_before_commit();
        assert!(lost.is_connection_lost());
        assert!(lost.to_string().contains("rolled the transaction back"));
        let server = DriverError::Unsupported("x".to_owned()).lost_before_commit();
        assert!(matches!(server, DriverError::Unsupported(_)));
    }
}
