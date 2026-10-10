// SPDX-License-Identifier: Apache-2.0
//! The typed, user-readable error every connection command rejects with.
//!
//! Messages never contain a password or a raw connection URL.
use openrows_core::{CoreError, ErrorKind};
use openrows_driver_api::DriverError;
use openrows_store::profile::{ProfileError, UrlError};
use serde::Serialize;
use specta::Type;

/// Failure category; the UI picks wording and recovery from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum IpcErrorKind {
    /// The application must be unlocked before accessing data.
    Locked,
    /// A field or identifier failed validation.
    Invalid,
    /// The profile or session does not exist.
    NotFound,
    /// Profile files could not be read or written.
    Storage,
    /// The OS keychain failed.
    Secret,
    /// The pasted connection URL is not a valid `postgres://` URL.
    Url,
    /// The server could not be reached or the connection dropped.
    Network,
    /// TLS files, certificate verification or the TLS handshake failed.
    Tls,
    /// The SSH tunnel could not be opened (connect, authentication, channel).
    Ssh,
    /// The SSH server's host key changed or was revoked: a hard stop.
    HostKey,
    /// The user cancelled a host-key or keyboard-interactive prompt.
    Cancelled,
    /// The server rejected the credentials.
    Auth,
    /// The server or driver broke the wire protocol.
    Protocol,
    /// The server reported an error (SQLSTATE in `code`).
    Server,
    /// The statement ran past the connection's statement timeout.
    Timeout,
    /// The requested option is not supported yet.
    Unsupported,
    /// A bug or broken invariant.
    Internal,
}

/// Typed command failure, safe to render to the user.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{message}")]
pub(crate) struct IpcError {
    /// Failure category.
    pub(crate) kind: IpcErrorKind,
    /// Human-readable message.
    pub(crate) message: String,
    /// Database error code (SQLSTATE), when the server sent one.
    pub(crate) code: Option<String>,
}

impl IpcError {
    pub(crate) fn new(kind: IpcErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            code: None,
        }
    }

    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self::new(IpcErrorKind::Invalid, message)
    }
}

impl From<ProfileError> for IpcError {
    fn from(error: ProfileError) -> Self {
        let kind = match &error {
            ProfileError::NotFound { .. } => IpcErrorKind::NotFound,
            ProfileError::Invalid { field, reason } => {
                return Self::invalid(format!("{} {reason}", field_label(field)));
            }
            ProfileError::AlreadyExists { .. } => IpcErrorKind::Invalid,
            ProfileError::Secret(_) => IpcErrorKind::Secret,
            _ => IpcErrorKind::Storage,
        };
        Self::new(kind, error.to_string())
    }
}

impl From<UrlError> for IpcError {
    fn from(error: UrlError) -> Self {
        Self::new(IpcErrorKind::Url, error.to_string())
    }
}

impl From<CoreError> for IpcError {
    fn from(error: CoreError) -> Self {
        let kind = match error.kind {
            ErrorKind::Network => IpcErrorKind::Network,
            ErrorKind::Tls => IpcErrorKind::Tls,
            ErrorKind::Auth => IpcErrorKind::Auth,
            ErrorKind::Protocol | ErrorKind::Decode => IpcErrorKind::Protocol,
            ErrorKind::Server => IpcErrorKind::Server,
            ErrorKind::Unsupported => IpcErrorKind::Unsupported,
            ErrorKind::Timeout => IpcErrorKind::Timeout,
            ErrorKind::UnknownSession | ErrorKind::SessionClosed => IpcErrorKind::NotFound,
            ErrorKind::QueueFull | ErrorKind::Internal => IpcErrorKind::Internal,
        };
        Self {
            kind,
            message: error.message,
            code: error.db_code,
        }
    }
}

impl From<&DriverError> for IpcError {
    fn from(error: &DriverError) -> Self {
        Self::from(CoreError::from(error))
    }
}

/// Field names as the connection form labels them.
fn field_label(field: &str) -> &str {
    match field {
        "name" => "Name",
        "params.host" => "Host",
        "params.port" => "Port",
        "ssh.host" => "SSH host",
        "ssh.port" => "SSH port",
        "ssh.keyFile" => "SSH key file",
        "ssh.jumpHosts" => "Jump hosts",
        "proxy" => "A proxy",
        "proxy.host" => "Proxy host",
        "proxy.port" => "Proxy port",
        "proxy.user" => "Proxy user",
        "tls.caFile" => "TLS CA:",
        "id" => "Connection id",
        "file.attached.alias" => "Schema name",
        "file.attached.path" => "Attached database file",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_validation_names_the_form_field() {
        let error = IpcError::from(ProfileError::Invalid {
            field: "params.host",
            reason: "must not be empty",
        });
        assert_eq!(error.kind, IpcErrorKind::Invalid);
        assert_eq!(error.message, "Host must not be empty");
    }

    #[test]
    fn server_errors_keep_their_sqlstate() {
        let error = IpcError::from(CoreError {
            kind: ErrorKind::Server,
            message: "database \"nope\" does not exist".to_owned(),
            db_code: Some("3D000".to_owned()),
        });
        assert_eq!(error.kind, IpcErrorKind::Server);
        assert_eq!(error.code.as_deref(), Some("3D000"));
    }
}
