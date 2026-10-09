// SPDX-License-Identifier: Apache-2.0
//! Engine error model: a cloneable subset of docs/04 §7 that front-ends can render.
use openrows_driver_api::DriverError;
use openrows_types::SessionId;
use std::time::Duration;

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
    /// The statement ran past its statement timeout and was cancelled (by
    /// the session or by the server); distinct from a user cancel.
    Timeout,
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

    /// The statement ran longer than `limit` and was cancelled.
    pub(crate) fn statement_timeout(limit: Duration) -> Self {
        Self::new(
            ErrorKind::Timeout,
            format!(
                "The statement was cancelled because it ran longer than the {} statement \
                 timeout of this connection.",
                seconds_label(limit)
            ),
        )
    }
}

/// `30 s`, `1.5 s` or `500 ms`.
fn seconds_label(limit: Duration) -> String {
    if limit < Duration::from_secs(1) {
        return format!("{} ms", limit.as_millis());
    }
    let seconds = limit.as_secs_f64();
    if seconds.fract() == 0.0 {
        format!("{} s", limit.as_secs())
    } else {
        format!("{seconds:.1} s")
    }
}

/// The server cancelled the statement at its own statement timeout:
/// `PostgreSQL` `statement_timeout` (57014 with that reason), `MySQL`
/// `max_execution_time` (`ER_QUERY_TIMEOUT`) or `MariaDB`
/// `max_statement_time` (`ER_STATEMENT_TIMEOUT`). The `MySQL` driver reports
/// SQLSTATEs, which both share with other errors, so the message decides.
fn is_server_timeout(code: &str, message: &str) -> bool {
    match code {
        "57014" => message.contains("statement timeout"),
        _ => {
            message.contains("maximum statement execution time exceeded")
                || message.contains("max_statement_time exceeded")
        }
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
                let kind = if is_server_timeout(code, message) {
                    ErrorKind::Timeout
                } else {
                    ErrorKind::Server
                };
                return Self {
                    kind,
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
    fn server_side_statement_timeouts_are_timeouts_not_cancels() {
        let server = |code: &str, message: &str| {
            CoreError::from(&DriverError::Server {
                code: code.to_owned(),
                message: message.to_owned(),
            })
        };
        let pg = server("57014", "canceling statement due to statement timeout");
        assert_eq!(pg.kind, ErrorKind::Timeout);
        assert_eq!(pg.db_code.as_deref(), Some("57014"));
        let user = server("57014", "canceling statement due to user request");
        assert_eq!(user.kind, ErrorKind::Server);
        assert_eq!(
            server("3024", "maximum statement execution time exceeded").kind,
            ErrorKind::Timeout
        );
        assert_eq!(
            server("1969", "max_statement_time exceeded").kind,
            ErrorKind::Timeout
        );
        let ours = CoreError::statement_timeout(Duration::from_secs(30));
        assert_eq!(ours.kind, ErrorKind::Timeout);
        assert!(ours.message.contains("30 s statement timeout"));
        assert_eq!(seconds_label(Duration::from_millis(1500)), "1.5 s");
        assert_eq!(seconds_label(Duration::from_millis(50)), "50 ms");
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
