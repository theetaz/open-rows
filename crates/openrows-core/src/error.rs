// SPDX-License-Identifier: Apache-2.0
//! Engine error model: a cloneable subset of docs/04 §7 that front-ends can render.
use openrows_driver_api::DriverError;
use openrows_types::SessionId;

/// Broad failure category; front-ends choose wording and recovery from it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    /// Transport failure; the connection is gone.
    Network,
    /// TLS setup, certificate verification or handshake failed.
    Tls,
    /// Credentials rejected or authentication method unsupported.
    Auth,
    /// The server or driver broke the wire protocol.
    Protocol,
    /// The database rejected the statement; `db_code` carries the SQLSTATE.
    Server,
    /// A value could not be decoded.
    Decode,
    /// The driver does not support the request.
    Unsupported,
    /// The session id is not open in this manager.
    UnknownSession,
    /// The session is closed, failed or shutting down.
    SessionClosed,
    /// The session's command queue is full; retry after earlier work finishes.
    QueueFull,
    /// A bug: a panicking driver or a broken invariant.
    Internal,
}

/// Typed engine failure, already safe to show to the user.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{message}")]
pub struct CoreError {
    /// Failure category.
    pub kind: ErrorKind,
    /// Human-readable message.
    pub message: String,
    /// Database error code (SQLSTATE or vendor code), when the server sent one.
    pub db_code: Option<String>,
}

impl CoreError {
    pub(crate) fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            db_code: None,
        }
    }

    pub(crate) fn unknown_session(id: SessionId) -> Self {
        Self::new(ErrorKind::UnknownSession, format!("unknown session {id}"))
    }

    pub(crate) fn session_closed(id: SessionId) -> Self {
        Self::new(ErrorKind::SessionClosed, format!("session {id} is closed"))
    }
}

impl From<&DriverError> for CoreError {
    fn from(error: &DriverError) -> Self {
        let kind = match error {
            DriverError::Network(_) => ErrorKind::Network,
            DriverError::Tls(_) => ErrorKind::Tls,
            DriverError::Auth(_) => ErrorKind::Auth,
            DriverError::Protocol(_) => ErrorKind::Protocol,
            DriverError::Server { code, message } => {
                return Self {
                    kind: ErrorKind::Server,
                    message: message.clone(),
                    db_code: Some(code.clone()),
                };
            }
            DriverError::Decode { .. } | DriverError::Arrow(_) => ErrorKind::Decode,
            DriverError::Unsupported(_) => ErrorKind::Unsupported,
        };
        Self::new(kind, error.to_string())
    }
}

/// True when the driver drained the connection and it can run the next statement.
pub(crate) fn keeps_session(error: &DriverError) -> bool {
    matches!(
        error,
        DriverError::Server { .. } | DriverError::Decode { .. } | DriverError::Unsupported(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_errors_keep_their_code_and_the_session() {
        let error = DriverError::Server {
            code: "42P01".to_owned(),
            message: "relation \"nope\" does not exist".to_owned(),
        };
        let core = CoreError::from(&error);
        assert_eq!(core.kind, ErrorKind::Server);
        assert_eq!(core.db_code.as_deref(), Some("42P01"));
        assert_eq!(core.message, "relation \"nope\" does not exist");
        assert!(keeps_session(&error));
    }

    #[test]
    fn transport_errors_lose_the_session() {
        let error = DriverError::Network(std::io::ErrorKind::BrokenPipe.into());
        assert_eq!(CoreError::from(&error).kind, ErrorKind::Network);
        assert!(!keeps_session(&error));
        let error = DriverError::Protocol("bad frame".to_owned());
        assert_eq!(CoreError::from(&error).kind, ErrorKind::Protocol);
        assert!(!keeps_session(&error));
    }
}
